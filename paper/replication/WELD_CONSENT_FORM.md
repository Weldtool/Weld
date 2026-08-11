# Informed Consent — Weld User Study

> **Filled for MTU IRB-2026-111 (Michigan Technological University).** This is the maintained source for the participant-facing consent text. The in-app version lives in `apps/web/src/lib/components/study/StudyIntro.svelte` and must be kept in sync. Do not launch until the current consent version is IRB-approved.

---

**Study title:** The effect of AI agent context scope on productivity and trust in an integrated developer workspace

**Researchers:** Dr. Jie Wu (Principal Investigator), Andrew Sadler (Student Investigator), Computer Science, Michigan Technological University

**Contact:** awsadler@mtu.edu, jwu2@mtu.edu · IRB contact: MTU Human Research Protection Program, irb@mtu.edu or (906) 487-1799

---

### What is this study about?

You're invited to take part in a research study about how developers work when their tools (task board, team chat, and code integration) are combined into a single application, and how access to an AI assistant within that workspace affects how quickly and comfortably people complete common development-coordination tasks.

### Who can take part?

Adults 18 or older with at least one year of programming experience who have used an issue tracker, a team chat tool, and Git/GitHub before, and who have not used Weld extensively prior to this study.

### What will I do?

If you agree, you'll complete a single online session of about **an hour** on your own:

1. Answer a few short background questions to confirm eligibility.
2. Work through a set of guided tasks inside the Weld application using a fictional practice project called PetTracker (no real code or accounts of yours are involved). Some tasks ask you to complete them manually; others ask you to use a built-in AI assistant to help.
3. Complete a short survey about your experience, including questions about task difficulty, usability, and your impressions of the AI assistant.

You may also choose — entirely optionally — to leave an email if you'd be willing to be contacted about future research related to this study.

**Note on the AI assistant.** You will be assigned to one of three experimental conditions that differ in how much information the AI assistant can access (e.g., just team chat, or chat plus the task board, or chat plus board plus GitHub). You will not be told which condition you are in during the session. After the study concludes and data collection is complete, the researchers can provide more detail about conditions if you request it.

### What information is collected?

- **Your identity for result linkage:** your email address and institution name, so your session data can be linked to you. These are stored securely and separately from your study responses; they are not included in anonymized analysis datasets.
- **How you use the app during the tasks:** which actions you take and when (e.g., opening a task card, sending a message, moving a card on the board, navigating between views), and basic technical info (browser, operating system, screen size).
- **Messages you post** as part of completing the tasks — these are your submitted outputs and are needed to score task correctness. They are stored as study data.
- **AI assistant interactions:** the messages you send to the AI assistant and its replies, along with which workspace surfaces the assistant was given access to in your session. The assistant is powered by a third-party language-model service (OpenAI): the messages you send it, and the practice-project content it is given to answer them, are transmitted to that service for processing under their API data terms (this content is not used to train their models). No real personal data or real code of yours is involved.
- **Your survey answers**, including ratings of effort, usability, and trust in the AI assistant, and a few open-ended comments.
- **Background categories** such as your age range and years of programming experience.
- **An anonymized participant code** (e.g., `P0012`) assigned to your session and shown to you at the end, which serves as your pseudonymous identifier in the dataset.

**We do NOT collect:** what you type keystroke-by-keystroke (only your final submitted messages are saved), your mouse movements, any screen or video recording, anything from your other apps or browser tabs, or any of your real personal projects.

### What if I am found ineligible?

If your screening answers indicate you don't meet the eligibility criteria, you'll see a polite notification and **no data of any kind will be collected or stored.**

### Are there any risks?

Risks are minimal — no more than ordinary computer use. You might feel mild fatigue or frustration during the tasks or survey. You can stop at any time by closing the browser tab.

### Are there any benefits?

There's no direct benefit to you. Your participation helps researchers understand how to design better development tools and AI-assisted workflows.

### Will I be paid?

There is no compensation for this study.

### How is my privacy protected?

- Your data is labeled with an anonymized participant code, not your name. Email and institution are stored in a separate, access-restricted table and are not included in exported analysis datasets.
- If you provide an email for future research related to this study, it is stored separately from your study data and deleted after that project concludes.
- The study dataset (behavioral events, survey responses) is stored on secure, access-restricted institutional systems and seen only by the research team — except that your messages to the AI assistant are processed by a third-party model service (OpenAI), as described above.
- Any quotes used in publications will be checked to remove identifying details.
- De-identified study data (containing no name, email, or institution) may be retained indefinitely and shared publicly — for example, deposited in a public research repository such as OSF or Zenodo — to support reproducibility and future research. Shared data contains no identifiers.

### Is participation voluntary?

Yes. Participation is completely voluntary. You may decline to answer any question and may stop at any time by closing the browser tab without penalty. If you stop partway through, or wish to withdraw afterward, you may contact us at awsadler@mtu.edu to have your data removed, as long as your participant code can still be used to identify your record. Data that has already been published, aggregated, or deposited in a public repository cannot be withdrawn.

### Who can I contact with questions?

About the study: awsadler@mtu.edu. About your rights as a research participant: contact the MTU Human Research Protection Program at irb@mtu.edu or (906) 487-1799.

---

### Consent

By checking the box below and clicking "I consent — continue," you confirm that:

- You are 18 years of age or older.
- You have read and understood this information.
- You understand that you will be randomly assigned to one of three AI assistant conditions and will not be told which condition during the session.
- You voluntarily agree to take part.

☐ I have read the above and consent to participate.

**[ I consent — continue ]** **[ No thanks ]**

*Consent version: v1.1. A record that you consented (the version identifier and timestamp) is stored with your session data. You may save or print a copy of this page for your records.*

---

### Implementation notes (not shown to participants)

- The in-app consent step is `step === 'consent'` in `StudyIntro.svelte`. The checkbox maps to `consentAffirmed`. Clicking "I consent — continue" advances to the screening step.
- Consent is recorded server-side on `POST /api/v1/study/provision` via the `consent_version` field, stored in `study_participants.consent_version` and `consent_given_at`. The telemetry event `consent_given` is also emitted client-side with `{ consent_version }`.
- **Ineligibility criteria (screening step):** `experience === 'lt1'` (< 1 yr programming) OR `familiarity === 'none'` (hasn't used all three: issue tracker + chat + Git/GitHub) OR `priorWeld === 'lots'` (extensive prior Weld use). Ineligible participants hit `study.markIneligible()` — no provision call is made, no data is stored.
- **Access codes:** when `STUDY_REQUIRE_ACCESS_CODE=true`, the screening step shows a `WELD-XXXX-XXXX` code field. An invalid or already-spent code fails provisioning. A previously-spent code belonging to the same participant resumes their session (not a new provision).
- The consent version string (`CONSENT_VERSION` in `apps/web/src/lib/study.svelte.ts`, currently `'v1.2'`) must be bumped whenever the consent language changes materially, and re-approval obtained.
  - **v1.1 (2026-06-16):** added third-party AI (OpenAI) processing disclosure, public/indefinite de-identified-data deposit + withdrawal-limit language, and corrected the "seen only by the research team" claim.
  - **v1.2 (2026-08-10):** corrected the stated session length from ~40 minutes to about an hour, to match the 12-task design.
  - Both are material changes. **IRB re-approval of the updated consent must be obtained before running participants**, and the Cayuse Security / Data-handling section must be updated to match (see `research-paper/IRB_TODO.md`, which is untracked).
