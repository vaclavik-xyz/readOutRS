## Code Review

Automatický AI code review běží po každém commitu (globální git hook).
- Reviews: `~/Dev/code-review/reviews/readOutRS/`
- Unresolved findings: `~/Dev/code-review/reviews/readOutRS/unresolved.md`
- Dismiss finding: `python3 ~/Dev/code-review/bin/review-dismiss.py add readOutRS --fingerprint {fingerprint} --reason "..."`
  - Fingerprint = 16-char hex z `unresolved.md` v `[hranatých závorkách]`, ne commit hash`
- Review prompt: `~/Dev/code-review/prompts/readOutRS.md`
- Prompt suggestions: `~/Dev/code-review/prompts/readOutRS.suggestions.md`

Po dokončení práce:
1. Zkontroluj `unresolved.md` — pokud tam jsou findings relevantní k tvým změnám, oprav je nebo dismiss s důvodem.
2. Zkontroluj `prompts/readOutRS.suggestions.md` — pokud existuje, zhodnoť návrhy na rozšíření review promptu. Relevantní návrhy přidej do `prompts/readOutRS.md` a odstraň je ze suggestions souboru.
