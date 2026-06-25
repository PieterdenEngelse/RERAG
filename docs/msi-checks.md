# MSI clean-VM verification checklist

End-to-end manual verification for the Windows MSI. Source of truth for
PR3.4 (unsigned MSI today) and the PR4 add-on once code signing lands.

The detailed prose lives in `docs/winsteps.md` §PR3.4 / §PR4; this file
is the copy-pasteable runbook to walk while sitting in front of a VM.

---

## 0. Pre-flight

- [ ] Fresh Windows 10 or 11 VM, no prior ag install
- [ ] Docker Compose available and Docker daemon running (`docker compose version` succeeds)
- [ ] Default SmartScreen settings (do not relax them)
- [ ] Logged in as a local Administrator (per-machine MSI needs this)
- [ ] Latest MSI downloaded from the release page:
      `ag-installer-vX.Y.Z-x86_64.msi`
- [ ] SHA256 matches `*.msi.sha256` from the release:
      ```pwsh
      (Get-FileHash .\ag-installer-vX.Y.Z-x86_64.msi -Algorithm SHA256).Hash.ToLower()
      Get-Content .\ag-installer-vX.Y.Z-x86_64.msi.sha256
      ```

## 1. MSI install

- [ ] Double-click the MSI
- [ ] **Unsigned build**: SmartScreen warns — click *More info* → *Run anyway*
- [ ] **Signed build (PR4)**: no SmartScreen popup; MSI dialog opens directly
- [ ] Walk the MSI dialog with all defaults; install completes without error
- [ ] No reboot prompt

## 2. MSI on-disk layout

The MSI is per-machine; everything below lives under `%PROGRAMFILES%\ag\`.

- [ ] `%ProgramFiles%\ag\bin\ag.exe` exists
- [ ] `%ProgramFiles%\ag\bin\ag-installer.exe` exists
- [ ] `%ProgramFiles%\ag\share\ag\docker-compose.yml` exists
- [ ] `%ProgramFiles%\ag\share\ag\.env.example` exists
- [ ] `%ProgramFiles%\ag\share\ag\scheduled-tasks\ag.xml.tmpl` exists
- [ ] `%ProgramFiles%\ag\share\ag\scheduled-tasks\ag-stack.xml.tmpl` exists
- [ ] `%ProgramFiles%\ag\share\ag\web\index.html` exists (+ `assets\…`
      with hashed `fro-*.js`, `fro_bg-*.wasm`, `output-*.css`)
- [ ] Start Menu → *RERAG* folder → *RERAG installer* shortcut present
- [ ] Both binaries answer `--version`:
      ```pwsh
      & "$env:ProgramFiles\ag\bin\ag.exe" --version
      & "$env:ProgramFiles\ag\bin\ag-installer.exe" --version
      ```

## 3. Run the GUI installer (Dioxus)

Launch *RERAG installer* from the Start Menu and walk every screen.

- [ ] **Welcome**: click Continue
- [ ] **Detection** rows show real values:
      - [ ] Docker: version string
      - [ ] Compose: *Down* (first install)
      - [ ] Ollama: *responding* or *not responding* (host-dependent)
      - [ ] FalkorDB: *not running*
      - [ ] ag.env: *missing*
      - [ ] Port 3010: *free*
      - [ ] System Redis: usually *not present*
      - [ ] Native obs row: hidden on Windows
      - [ ] ag task drift: *false*
      - [ ] Disk free / RAM: real GB values
      - [ ] Distro: e.g. *Windows 11 23H2*
- [ ] **Prompts**: accept defaults, Continue
- [ ] **Progress**: all six steps complete (green)
- [ ] *Open log* link works; log written to `%LOCALAPPDATA%\ag\logs\install-*.log`

## 4. Per-user runtime layout

The Dioxus installer (not the MSI) populates these on first run.

- [ ] `%LOCALAPPDATA%\ag\bin\ag.exe` exists (copied from `%ProgramFiles%`)
- [ ] `%LOCALAPPDATA%\ag\bin\ag-start.cmd` exists
- [ ] `%APPDATA%\ag\ag.env` exists
- [ ] `%APPDATA%\ag\docker-compose.yml` exists
- [ ] `%LOCALAPPDATA%\ag\logs\` exists

## 5. Scheduled tasks

```pwsh
schtasks /Query /TN ag
schtasks /Query /TN ag-stack
```

- [ ] Both tasks exist (no `ERROR: The system cannot find the file specified.`)
- [ ] `ag` task: logon trigger present
- [ ] `ag-stack` task: logon trigger present

## 6. Compose stack

```pwsh
docker compose ls         # expect project "ag", status running
docker ps                 # expect ag-falkordb, ag-redis containers up
```

- [ ] `docker compose ls` lists `ag` with status `running`
- [ ] `docker ps` shows the expected ag-* containers healthy

## 7. First run — dashboard

- [ ] Browse to `http://127.0.0.1:3010` — RERAG dashboard renders
- [ ] Drop a small PDF into upload → ingest completes without error
- [ ] Search returns the ingested chunk
- [ ] Monitoring page shows non-zero Prometheus samples

## 8. Reboot / logon trigger

- [ ] Log out, log back in (no manual start)
- [ ] `http://127.0.0.1:3010` still serves the dashboard
- [ ] `docker compose ls` still shows ag project running
- [ ] Verifies `ag-start.cmd` fired from the scheduled-task logon trigger

## 9. Uninstall via installer CLI

```pwsh
& "$env:LOCALAPPDATA\ag\bin\ag-installer.exe" --uninstall --purge
```

- [ ] Command completes without error
- [ ] `schtasks /Query /TN ag` returns ERROR
- [ ] `schtasks /Query /TN ag-stack` returns ERROR
- [ ] `docker compose ls` no longer shows the `ag` project
- [ ] `%LOCALAPPDATA%\ag\` removed
- [ ] `%APPDATA%\ag\` removed

## 10. Uninstall via Apps & Features

- [ ] Settings → Apps & installed apps → *RERAG installer* → Uninstall
- [ ] `%ProgramFiles%\ag\` removed
- [ ] Start Menu *RERAG* folder removed
- [ ] No leftover entries in `schtasks /Query`

---

## Signed-build extras (PR4 only)

Skip this section until PR4 lands. Once the MSI is signed:

- [ ] No SmartScreen popup at step 1 (verified on a VM with default
      SmartScreen settings)
- [ ] Authenticode status for each binary:
      ```pwsh
      Get-AuthenticodeSignature "$env:ProgramFiles\ag\bin\ag.exe"          | Select Status, SignerCertificate
      Get-AuthenticodeSignature "$env:ProgramFiles\ag\bin\ag-installer.exe" | Select Status, SignerCertificate
      Get-AuthenticodeSignature ".\ag-installer-vX.Y.Z-x86_64.msi"         | Select Status, SignerCertificate
      ```
      Expected: `Status=Valid`, `SignerCertificate` matches the release cert
- [ ] Timestamp present (`SignerCertificate.NotAfter` aside —
      `TimeStamperCertificate` set on each `Get-AuthenticodeSignature`
      result)
- [ ] If SmartScreen still warns despite `Status=Valid`: OV-cert
      reputation issue, not a signing bug. Document in README until
      reputation builds, or move to an EV cert.

---

## Pass criterion

Every box in §0–§10 ticks without manual intervention beyond accepting
SmartScreen on first run (unsigned build) or none at all (signed build).
Any unticked box blocks the release tag from being promoted from
*prerelease* to *latest*.
