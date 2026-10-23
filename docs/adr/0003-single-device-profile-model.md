# Single-Device Profile Model with Encrypted Backup

We decided to adopt a single-device-first architecture where an anonymous profile and its pairwise queues are owned exclusively by one physical device, using encrypted database export/import for profile migration. Multi-device synchronized messaging across unidirectional message queues introduces race conditions, state replication overhead, and metadata correlation vectors; single-device ownership keeps queue state completely deterministic for the initial release.
