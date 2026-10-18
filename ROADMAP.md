# nas-os roadmap

Remaining work and its intended order. Completed work is recorded in the
[implementation log](docs/development/IMPLEMENTATION.md). Behavior and contracts are documented in
[DEVELOPMENT.md](DEVELOPMENT.md), [OpenAPI](docs/development/openapi.json),
[API.md](docs/development/API.md), and [APPS.md](docs/development/external_apps/APPS.md).

A step is complete when it satisfies its acceptance criterion.

## S. System

### S1. Application network permissions

- [ ] Administrator-granted per-application network permissions for ports beyond HTTP and HTTPS.

**Complete when** each application can use only its authorized ports, and revoking permission
blocks them without interrupting `apt update` or application-update checks.
