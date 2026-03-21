# Systemd unit for Agentic RAG full stack

1. Copy the service into your user systemd directory:
   ```bash
   mkdir -p ~/.config/systemd/user
   cp /home/pde/ag/systemd/ag.service ~/.config/systemd/user/
   ```
2. Reload user-level systemd:
   ```bash
   systemctl --user daemon-reload
   ```
3. Enable auto-start at login:
   ```bash
   systemctl --user enable ag.service
   ```
4. Start immediately:
   ```bash
   systemctl --user start ag.service
   ```
This service runs `start-ag.sh`, which ensures the Docker compose stack (neo4j, redis, tempo, loki, prometheus, grafana, otel-collector) is up before launching the backend binary.
