# Data Model — DynamoDB Single Table

See [`CLAUDE.md` §5 — DynamoDB Single-Table Design](../CLAUDE.md#5-dynamodb-single-table-design) for the canonical item shapes, key construction, and access patterns.

## Index summary

| Index   | Partition key | Sort key |
|---------|---------------|----------|
| Primary | `PK`          | `SK`     |
| GSI1    | `GSI1PK`      | `GSI1SK` |

All access in production code paths must be `GetItem` or `Query`. **No scans.**
