# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in this project, please report it responsibly.

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Instead, send an email to **emersonalmeidax@gmail.com** with:

- A description of the vulnerability
- Steps to reproduce the issue
- The potential impact
- Any suggested fixes (optional)

## Response Timeline

- **Acknowledgment**: within 48 hours
- **Initial assessment**: within 1 week
- **Fix or mitigation**: as soon as possible, depending on severity

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅ Yes    |

## Security Practices

This project follows these security practices:

- API keys are hashed with SHA-256 before storage (never stored in plaintext)
- Rate limiting is enforced on all authenticated endpoints
- All database queries use parameterized statements (no SQL injection)
- Input validation on all user-provided data
- CORS headers are set explicitly

Thank you for helping keep Agent Todo secure!
