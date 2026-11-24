# Security policy

## Supported versions

Only the `main` branch is maintained.

## Reporting a vulnerability

**Do not open a public issue.** Use GitHub's private reporting instead: the [Security](https://github.com/EGCGAP04/phonengine/security) tab -> *Report a vulnerability*.

If you can, include:

- What is affected (the core library, DSL parser, CLI, or a workflow).
- How to reproduce it and what impact it has.

## Scope

`phonengine` is a deterministic offline library and CLI with no network services, accounts, or stored user credentials. Reports about parsing bugs that cause unbounded resource consumption, crashes, or unsafe behavior on untrusted rule/lexicon files are welcome.
