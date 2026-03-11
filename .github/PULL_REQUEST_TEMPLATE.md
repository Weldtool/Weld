# Security Policy

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

If you discover a security issue in Weld, we appreciate your help in disclosing it responsibly. Here's how:

### Option 1: GitHub Private Vulnerability Reporting (Preferred)

Use GitHub's built-in private reporting feature:

1. Go to the **Security** tab of this repository
2. Click **"Report a vulnerability"**
3. Fill in the details

This keeps the report private between you and the maintainers.

### Option 2: Email

Send an email to **security@{yourdomain}** with:

- A description of the vulnerability
- Steps to reproduce it
- The potential impact
- Any suggested fixes (optional but appreciated)

## What to Expect

- **Acknowledgment** within 48 hours that we received your report
- **Assessment** within 7 days with an initial severity evaluation
- **Resolution timeline** communicated to you based on severity
- **Credit** in our release notes (unless you prefer to stay anonymous)

## Scope

The following are in scope for security reports:

- Authentication and authorization bypasses
- Data exposure or leaks
- Injection vulnerabilities (SQL, XSS, etc.)
- Cryptographic weaknesses (secrets management, encryption)
- Server-side request forgery
- Privilege escalation
- WebSocket security issues

The following are **out of scope**:

- Denial of service (DoS) attacks
- Social engineering
- Issues in third-party dependencies (report those upstream, but let us know)
- Issues requiring physical access to a user's device

## Supported Versions

| Version  | Supported |
| -------- | --------- |
| latest   | ✅        |
| < latest | ❌        |

We only patch the latest release. Users are encouraged to stay up to date.

## Safe Harbor

We consider security research conducted in good faith to be authorized. We will not pursue legal action against researchers who:

- Act in good faith to avoid privacy violations, data destruction, or service disruption
- Only interact with accounts they own or with explicit permission
- Report vulnerabilities promptly and don't exploit them beyond what's needed to demonstrate the issue
