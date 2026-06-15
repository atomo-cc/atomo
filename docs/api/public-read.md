---
title: Public Read (REST)
description: A narrow, default-deny anonymous read surface for explicitly approved projection models.
---

# Public Read (REST)

A single read-only endpoint exposes **explicitly approved** projection models to anonymous clients
without granting access to GraphQL, mutations, or any other model. It is **default deny**: a model
is reachable only when an operator allowlists it *and* its schema declares public read, and even then
only the rows and query fields the operator configures are exposed.

```http
GET /public/records/{model}
GET /public/records/{model}?{allowed_field}={value}&limit={1..100}
```

Response:

```json
{ "items": [ { "...": "approved projection fields" } ] }
```

## Dual approval (both required)

1. **Operator allowlist** — the model name appears in `ATOMO_PUBLIC_READ_MODELS`.
2. **Schema declaration** — the model declares `read: allow.public()` (the GraphQL resolver still
   evaluates this rule).

A model missing either is `404`, indistinguishable from an unknown model. There is no mutation
surface and no way to run arbitrary GraphQL through this route.

## Per-model policy (no assumed conventions)

The route assumes no `status` or `slug` convention. Each model declares its own policy:

| Variable | Meaning | Example |
| --- | --- | --- |
| `ATOMO_PUBLIC_READ_MODELS` | Comma list of allowlisted models | `PublicListing,PublicArticle` |
| `ATOMO_PUBLIC_READ_FILTER_<Model>` | Fixed equality filters always applied (`field:value`, comma-separated) | `status:published` |
| `ATOMO_PUBLIC_READ_FIELDS_<Model>` | Query fields a client may filter on (comma-separated) | `slug,category` |

- **Fixed filters win.** They are applied after client filters, so a client cannot override a forced
  filter (e.g. cannot flip a forced `status:published` to `draft`).
- **Only listed query fields are honored.** Any other query parameter is ignored, so a client can
  never widen the exposed rows. A model with no `FIELDS` config accepts no client filters.
- **No filter config = all public rows.** Restricting to e.g. published rows is the operator's
  explicit choice via `ATOMO_PUBLIC_READ_FILTER_<Model>`.
- `limit` is clamped to `1..=100` (default `100`).

> These keys are per-model and dynamic, so they are read from the environment directly rather than
> held as static `ServerConfig` fields (consistent with `ATOMO_PUBLIC_READ_MODELS`).

## Example

For a published-listing projection queryable by slug or category:

```bash
ATOMO_PUBLIC_READ_MODELS=PublicListing
ATOMO_PUBLIC_READ_FILTER_PublicListing=status:published
ATOMO_PUBLIC_READ_FIELDS_PublicListing=slug,category
```

```http
GET /public/records/PublicListing?slug=logo-maker
GET /public/records/PublicListing?category=design&limit=20
```

Both return only published rows; a request for `?status=draft` is ignored (status is forced and not
an allowed query field).
