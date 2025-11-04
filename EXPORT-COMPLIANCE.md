# Export Compliance Notice

Durapack is publicly available open‑source software licensed under MIT/Apache‑2.0. The project provides a self‑locating framing format and recovery tools intended for general‑purpose use (e.g., spaceflight telemetry, embedded logging, lossy/intermittent networks).

Current status (as of this commit)

- No confidentiality or encryption for secrecy is implemented.
- An optional BLAKE3 trailer is used for integrity only.
- Any cryptographic or authenticity features (e.g., signatures) are not enabled by default and will be behind explicit feature flags.
- The project is intended for unclassified, non‑controlled usage. It is not intended to store, process, or transmit classified or export‑controlled technical data.

Export control guidance

- Based on current functionality, the project is expected to self‑classify as EAR99 (subject to applicable law). This is not a formal legal determination.
- Adding confidentiality encryption or certain cryptographic implementations may change export classification and could require licensing, notification, or a formal classification request to the Bureau of Industry and Security (BIS) or the Directorate of Defense Trade Controls (DDTC).
- Users and contributors are responsible for ensuring they do not commit or publish export‑controlled or classified data into this repository.

Disclaimer

This document is informational and not legal advice. For a definitive export classification or if you plan to add encryption/confidentiality features, consult qualified export‑controls counsel or submit a formal classification request to the relevant US authorities.
