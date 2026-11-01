# Native offer, preparation and expiry integration

Assembled source `65f8eaa` passed28 tests across native offers, expiry,
preparation and NeverGranted, with no failures or ignores. Strict all-feature
lint for the library/offer/expiry targets and workspace formatting passed.
[Commands, exact source and retained logs](evidence/aq08-offer-expiry-65f8eaa/artifact-sha256.json)
bind this result separately from the source component checkpoints.

The added cross-component case obtains an offer through the mounted publisher,
accepts it through the native machine route, and runs the exact expiry owner at
the original preparation deadline. It verifies NeverGranted closure and material
revocation, retained inline artifacts and decodable original Git export linkage,
and absence of issued agent credentials. It does not claim physical stop, source
settlement or local workspace release.

The first test attempt advanced a fixture AtomicI64 that the publication fixture
had replaced with a real clock. Correctly, the preparation remained fresh and
held. The retained negative is superseded by a test that clones only the cleanup
state with the original absolute expiry as its clock; no receipt or deadline is
rewritten. Publication and acceptance still use actual HTTP-owner timestamps.

These are actual disposable SQL/router operations with synthetic external facts.
The first publisher remains one slot/first selected project with the documented
short cold-preparation window. Grants, renewal, full worker execution and the
granted-without-Started recovery path remain separate work. The existing browser
preview still runs the separately qualified schema27/playbook-startup binary;
this checkpoint does not silently attribute a live-browser run to newer source.
