# AQ-08 stage 1 evidence

Revision `3520832` adds independent bounded admission contracts and scoped public graph readers. Twelve pure contract/placement checks and three fake Graph HTTP checks pass, along with strict all-target Clippy and formatting. The extracted package repeats the twelve pure checks with its separately pinned public session contract; no daemon, graph database or sibling implementation is required.

The checks cover exact source/material/artifact bindings, placement fairness and fit, stale task/workspace/header responses, bounded paging and body sizes, duplicate JSON keys, truncation and exact retained fractional metadata bytes. Root reviewed the staged design, concrete authority bridge proposal and reader code. The companion JSON identifies frozen source and logs. Development package logs precede the commit and are not a published clean release artifact.

AQ-08 remains in progress. This increment observes candidates and proposes placements; it creates no execution claim, provider credential delivery or real process grant. Migration, common authority integration and actual profiled transport are separate follow-on work. Subsequent reader-bound corrections will be qualified with that work.
