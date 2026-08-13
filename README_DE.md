# CC MiniRoute

Windows-orientierte Desktop-Anwendung zur Verwaltung von Konfigurationen und
lokalem Routing fuer Codex, Claude, Gemini und verwandte Coding-Agenten.

[English](README.md) | [中文](README_ZH.md) | [日本語](README_JA.md) |
[Changelog](CHANGELOG.md)

## Ueber diesen Fork

CC MiniRoute basiert auf CC Switch und behaelt die Kompatibilitaet mit dessen
Provider-, MCP-, Skills-, Sitzungs- und Proxy-Arbeitsablaeufen. Der Schwerpunkt
dieses Forks liegt auf:

- Name, Anwendungskennung, Symbolen, Repository-Links und Update-Metadaten von
  CC MiniRoute.
- Windows-Desktop-Nutzung und Kompatibilitaet mit vorhandenen lokalen Daten.
- Codex-Provider- und Modellkatalogen, einschliesslich nativer Responses-API-
  Modelle anderer Anbieter.
- Lokalen Routing-Experimenten, die unabhaengig vom Upstream getestet werden.

Dieses Repository bewirbt oder empfiehlt keine API-Relay-Anbieter. Provider-
Vorlagen sind ausschliesslich Konfigurationsvorlagen. Qualitaet, Preise,
Datenschutz und Kontosicherheit muessen vom Benutzer geprueft werden.

## Hauptfunktionen

- Provider-Verwaltung fuer Codex, Claude Code, Claude Desktop, Gemini CLI,
  OpenCode, OpenClaw, Grok Build und Hermes.
- Umschalten von Konfigurationen ueber die Desktop-Oberflaeche oder das Tray.
- Lokaler Proxy mit Routing, Protokollanpassung, Failover und Diagnose.
- Verwaltung von MCP-Servern, Prompts, Skills, Nutzung und lokalen Sitzungen.
- Sicherung vor verwalteten Aenderungen und Wiederherstellung beim Abschalten.
- Kombinierter Codex-Modellkatalog fuer native Responses-API-Provider.

## Experimenteller stabiler Codex-Proxy-Einstieg

Der optionale stabile Einstieg haelt Codex an einem lokalen `custom` Responses-
API-Endpunkt, waehrend CC MiniRoute das eigentliche Upstream-Ziel intern
wechselt. Dadurch sollen Neustarts und wiederholte Aenderungen der aktiven
Codex-Konfiguration reduziert werden.

Die Option ist standardmaessig deaktiviert. Testen Sie sie zuerst auf einem
unkritischen System. Modellkompatibilitaet, Tool-Aufrufe, Kontextverarbeitung
und Abrechnung bleiben Eigenschaften des ausgewaehlten Upstream-Dienstes.

## Aus dem Quellcode bauen

```powershell
pnpm install
pnpm typecheck
pnpm tauri build --bundles nsis,msi
```

Windows-Pakete werden hier erzeugt:

```text
src-tauri/target/release/bundle/
```

## Upstream und Lizenz

CC MiniRoute basiert auf
[CC Switch](https://github.com/farion1231/cc-switch) von farion1231 und den
Mitwirkenden. Das Projekt steht unter der MIT License; siehe [LICENSE](LICENSE).
