# Disabled upstream automation

These workflows are preserved from Tempo outside `.github/workflows`, so GitHub
does not execute them. See [the CI inventory](../../docs/ci.md) for the reason for
each group, checks retained in Bankd, and what is needed before restoring a job.

Some files call other workflows by their original paths. Restore and adapt the
whole dependency chain before moving a workflow back into `.github/workflows`.
