rtex upstream `98b40be29ef517918ce707869e6fe0316685b3dd`; latest tag `v0.0.6`; pin `v0.0.6` (`98b40be29ef517918ce707869e6fe0316685b3dd`)

| stage | result | detail |
| --- | --- | --- |
| latest tag vs pin | same | v0.0.6 (98b40be29ef517918ce707869e6fe0316685b3dd) |
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
