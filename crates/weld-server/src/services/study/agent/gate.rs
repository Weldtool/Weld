//! The Context Gate (CLAUDE_AGENT_BUILD.md §2–§3): the single chokepoint that
//! assembles the agent's context. Scoping is a hard boundary enforced here in
//! code, never a prompt instruction — disallowed sources are simply never
//! queried or included. The assembled snapshot is logged (`agent_context_sent`)
//! so scoping is provable from logs after the fact.
//!
//! The one rule that matters most: a condition only ever receives the context
//! its allowlist permits. A C1 agent must NEVER see board or GitHub data; C2
//! must never see GitHub.

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::ParticipantCtx;
use crate::errors::ApiError;
use crate::services::study::github_fixtures;

/// Between-subjects agent condition, fixed per participant for the whole session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentCondition {
    /// Chat only; advise-only (no tools).
    C1,
    /// Chat + board; may act on board/chat.
    C2,
    /// Chat + board + GitHub; may also link cards to issues/PRs.
    C3,
}

impl AgentCondition {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        match s {
            "C1" => Ok(Self::C1),
            "C2" => Ok(Self::C2),
            "C3" => Ok(Self::C3),
            other => Err(ApiError::Internal(format!(
                "Unknown agent condition: {other}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::C1 => "C1",
            Self::C2 => "C2",
            Self::C3 => "C3",
        }
    }
}

/// Which context sources a condition is allowed to receive. Default-deny: a
/// source not set to `true` here is never assembled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextRules {
    pub chat: bool,
    pub board: bool,
    pub github: bool,
}

/// The scoping allowlist — the heart of the study's validity.
pub fn rules_for(condition: AgentCondition) -> ContextRules {
    match condition {
        AgentCondition::C1 => ContextRules {
            chat: true,
            board: false,
            github: false,
        },
        AgentCondition::C2 => ContextRules {
            chat: true,
            board: true,
            github: false,
        },
        AgentCondition::C3 => ContextRules {
            chat: true,
            board: true,
            github: true,
        },
    }
}

/// Tool capability categories an agent may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCategory {
    BoardWrite,
    ChatPost,
    GithubLink,
}

/// Tools allowed per condition (capability scales with context by design). C1
/// is advise-only.
pub fn allowed_tool_categories(condition: AgentCondition) -> &'static [ToolCategory] {
    match condition {
        AgentCondition::C1 => &[],
        AgentCondition::C2 => &[ToolCategory::BoardWrite, ToolCategory::ChatPost],
        AgentCondition::C3 => &[
            ToolCategory::BoardWrite,
            ToolCategory::ChatPost,
            ToolCategory::GithubLink,
        ],
    }
}

// ── Assembled, scoped context ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ChatLine {
    pub channel: String,
    pub sender: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct CardLine {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub column: String,
    pub assignee: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub github_issue: Option<i64>,
    pub github_pr: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GithubItem {
    pub kind: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct ScopedContext {
    pub condition: String,
    pub sources_included: Vec<String>,
    pub chat: Vec<ChatLine>,
    pub board: Option<Vec<CardLine>>,
    pub github: Option<Vec<GithubItem>>,
}

impl ScopedContext {
    /// The proof-of-scoping artifact stored in `agent_context_sent`. For C1 the
    /// `board`/`github` keys are absent; for C2 `github` is absent.
    pub fn snapshot(&self) -> Value {
        let mut obj = json!({
            "condition": self.condition,
            "sources_included": self.sources_included,
            "chat": self.chat,
        });
        if let Some(board) = &self.board {
            obj["board"] = json!(board);
        }
        if let Some(github) = &self.github {
            obj["github"] = json!(github);
        }
        obj
    }

    /// Render the scoped context as text for the model prompt. Only included
    /// sources appear; nothing else can leak in because nothing else was assembled.
    pub fn render_for_prompt(&self) -> String {
        let mut out = String::new();

        out.push_str("## Team chat\n");
        if self.chat.is_empty() {
            out.push_str("(no messages)\n");
        }
        for line in &self.chat {
            out.push_str(&format!(
                "[#{}] {}: {}\n",
                line.channel, line.sender, line.content
            ));
        }

        if let Some(board) = &self.board {
            out.push_str("\n## Board cards\n");
            for c in board {
                let assignee = c.assignee.as_deref().unwrap_or("unassigned");
                out.push_str(&format!(
                    "- ({}) \"{}\" [col: {}, priority: {}, assignee: {}, labels: {}, id: {}]\n",
                    c.column,
                    c.title,
                    c.column,
                    c.priority,
                    assignee,
                    c.labels.join("/"),
                    c.id,
                ));
                if let Some(desc) = &c.description {
                    out.push_str(&format!("    {desc}\n"));
                }
            }
        }

        if let Some(github) = &self.github {
            out.push_str("\n## GitHub (read-only)\n");
            for g in github {
                out.push_str(&format!(
                    "- {} #{} \"{}\" ({}): {}\n",
                    g.kind, g.number, g.title, g.state, g.body
                ));
            }
        }

        out
    }
}

/// Assemble the scoped context for a participant. THE chokepoint: it queries
/// only the sources the condition allows, always from the participant's own
/// isolated team/project.
pub async fn assemble(pool: &PgPool, ctx: &ParticipantCtx) -> Result<ScopedContext, ApiError> {
    let rules = rules_for(ctx.condition);
    let mut sources_included = Vec::new();

    // Chat — allowed in every condition. Raw content (incl. any [[card:..]]
    // tokens) is passed through; C1 gets no board data to resolve them against.
    let chat = if rules.chat {
        sources_included.push("chat".to_string());
        assemble_chat(pool, ctx.team_id).await?
    } else {
        Vec::new()
    };

    let board = if rules.board {
        sources_included.push("board".to_string());
        Some(assemble_board(pool, ctx.project_id).await?)
    } else {
        None
    };

    let github = if rules.github {
        sources_included.push("github".to_string());
        Some(assemble_github(pool, ctx).await?)
    } else {
        None
    };

    Ok(ScopedContext {
        condition: ctx.condition.as_str().to_string(),
        sources_included,
        chat,
        board,
        github,
    })
}

async fn assemble_chat(pool: &PgPool, team_id: Uuid) -> Result<Vec<ChatLine>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT ch.name AS channel, u.display_name AS sender, m.content
        FROM messages m
        JOIN channels ch ON ch.id = m.channel_id
        JOIN users u ON u.id = m.sender_id
        WHERE ch.team_id = $1 AND m.deleted_at IS NULL AND ch.deleted_at IS NULL
        ORDER BY m.created_at ASC
        LIMIT 200
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ChatLine {
            channel: r.channel,
            sender: r.sender,
            content: r.content,
        })
        .collect())
}

async fn assemble_board(pool: &PgPool, project_id: Uuid) -> Result<Vec<CardLine>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.description, c.name AS column_name,
               t.priority::text AS "priority!", u.display_name AS "assignee?",
               t.github_issue_id, t.github_pr_id,
               COALESCE(array_agg(l.name) FILTER (WHERE l.name IS NOT NULL), '{}') AS "labels!"
        FROM tasks t
        JOIN columns c ON c.id = t.column_id
        LEFT JOIN users u ON u.id = t.assignee_id
        LEFT JOIN task_labels tl ON tl.task_id = t.id
        LEFT JOIN labels l ON l.id = tl.label_id
        WHERE t.project_id = $1 AND t.deleted_at IS NULL
        GROUP BY t.id, c.name, u.display_name, c.position, t.position
        ORDER BY c.position, t.position
        "#,
        project_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| CardLine {
            id: r.id,
            title: r.title,
            description: r.description,
            column: r.column_name,
            assignee: r.assignee,
            priority: r.priority,
            labels: r.labels,
            github_issue: r.github_issue_id,
            github_pr: r.github_pr_id,
        })
        .collect())
}

async fn assemble_github(pool: &PgPool, ctx: &ParticipantCtx) -> Result<Vec<GithubItem>, ApiError> {
    let issues = github_fixtures::list_issues(pool, ctx.project_id, ctx.user_id).await?;
    let prs = github_fixtures::list_prs(pool, ctx.project_id, ctx.user_id).await?;

    let mut out = Vec::new();
    for (kind, list) in [("issue", issues), ("pull_request", prs)] {
        for v in list {
            out.push(GithubItem {
                kind: kind.to_string(),
                number: v.get("number").and_then(Value::as_i64).unwrap_or_default(),
                title: v
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                state: v
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                body: v
                    .get("body")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c1_sees_only_chat() {
        let r = rules_for(AgentCondition::C1);
        assert!(r.chat);
        assert!(!r.board, "C1 must never receive board context");
        assert!(!r.github, "C1 must never receive GitHub context");
    }

    #[test]
    fn c2_sees_chat_and_board_but_not_github() {
        let r = rules_for(AgentCondition::C2);
        assert!(r.chat);
        assert!(r.board);
        assert!(!r.github, "C2 must never receive GitHub context");
    }

    #[test]
    fn c3_sees_everything() {
        let r = rules_for(AgentCondition::C3);
        assert!(r.chat && r.board && r.github);
    }

    #[test]
    fn c1_has_no_tools() {
        assert!(allowed_tool_categories(AgentCondition::C1).is_empty());
    }

    #[test]
    fn github_link_only_in_c3() {
        assert!(!allowed_tool_categories(AgentCondition::C2).contains(&ToolCategory::GithubLink));
        assert!(allowed_tool_categories(AgentCondition::C3).contains(&ToolCategory::GithubLink));
    }

    #[test]
    fn snapshot_omits_disallowed_sources_for_c1() {
        let scoped = ScopedContext {
            condition: "C1".into(),
            sources_included: vec!["chat".into()],
            chat: vec![ChatLine {
                channel: "pettracker".into(),
                sender: "Sarah".into(),
                content: "hi".into(),
            }],
            board: None,
            github: None,
        };
        let snap = scoped.snapshot();
        assert!(
            snap.get("board").is_none(),
            "C1 snapshot must not contain board"
        );
        assert!(
            snap.get("github").is_none(),
            "C1 snapshot must not contain github"
        );
        assert!(snap.get("chat").is_some());
    }
}
