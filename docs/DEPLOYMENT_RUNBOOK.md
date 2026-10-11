# Production deployment runbook

This file records the release path that has actually delivered this project. Read it before attempting any production deployment or asking the owner for connection details.

## Production identity

- Public URL: `https://nddev.link`
- Stand: DigitalOcean droplet `fsp-career-demo`, origin `188.166.88.183`
- Active release: `/opt/fsp/current`
- Do not use `178.154.244.130` or `/home/dragan/vopstchage`; that is an unrelated project.

## Normal release path

Previous successful releases did not use a workstation SSH key. After a checked change reached `main`, the private `fsp-ci` controller on the authorized CI host:

1. ran `fsp/platform`, `fsp/backend`, `fsp/compose` and `fsp/release-safety`;
2. invoked `infra/deploy.sh <main-sha>` with `FSP_DEPLOY_HOST=stand` and the root-only `/etc/fsp-ci/ssh-config`;
3. ran the real public browser/mail acceptance against `nddev.link`;
4. wrote `/var/lib/fsp-ci/deployed.json` and published `fsp/deployment=success` only after acceptance passed.

The accepted release is proved only when canonical `main`, `/opt/fsp/current`, `deployed.json` and the successful `fsp/deployment` status contain the same SHA. A merge, public HTTP 200, local tests or an idle successful CI cycle is not deployment proof.

## Recovery rule

If the controller does not report statuses, inspect its documented state before trying manual SSH. Do not ask the owner to reconstruct the deployment process and do not invent a new server route. Current recovery is tracked in [issue #161](https://github.com/KirpaDmitriy/sport-programming-federation/issues/161).

The former active CI droplet `server-gha-runner-1` was destroyed by DigitalOcean action `3463766117` on 2026-10-11. Its root-only stand credential and controller state were on that host. The local key `~/.ssh/id_ed25519_vopshage_deploy` is not authorized on the stand; retrying it is not a recovery plan.

Restore the private controller on an owner-approved destination with the procedure in `infra/ci/README.md`, or recover the private CI configuration through the authorized cloud console. Then let the controller deploy the exact `main` SHA. Never fabricate statuses or `deployed.json`.

## TTS activation

The API reads speech provider configuration from `/etc/fsp/llm.env` through `fsp-api.service`:

- xAI: `FSP_TTS_PROVIDER=xai` and `XAI_API_KEY=...`;
- Yandex SpeechKit: `FSP_TTS_PROVIDER=yandex` and `YANDEX_SPEECHKIT_API_KEY=...`.

Keep provider keys out of Git, issues and chat. Without either configuration the frontend deliberately uses browser speech synthesis.

