rtex upstream `origin/main` (`a27561fdd6ec913f2a80d5b264f8fa7aaa228478`); pin `main` (`a27561fdd6ec913f2a80d5b264f8fa7aaa228478`)

| stage | result | detail |
| --- | --- | --- |
| upstream commit vs pin | same | a27561fdd6ec913f2a80d5b264f8fa7aaa228478 |
| commits since pin | info | 0; see below and commits.log |
| build | ok | exit 0 |
| cargo deny licenses-sources | ok | exit 0; unfiltered log |
| cargo deny bans | findings | exit 2; unfiltered log |
| cargo deny advisories | findings | exit 1; unfiltered log |
| unprivileged user namespaces | ok | bwrap exit 0; no sysctl override |
| protocol smoke | ok | serve + one edit + own update + layout |
| range rejection | ok | E8 subset; server stays alive |
| path rejection | ok | E8 subset; server stays alive |
| E2 negative read | ok | host effective=True; sandbox denied=True |

Commits since pin:
- None (or unavailable; see setup result)
