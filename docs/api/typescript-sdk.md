# TypeScript SDK

The `@atomo-cc/client-sdk` package provides typed client utilities for frontend apps.

## Install

```bash
pnpm add @atomo-cc/client-sdk
```

## Quick Start

```ts
import { createClient } from '@atomo-cc/client-sdk'

const atomo = createClient({ endpoint: 'http://localhost:3000/graphql' })

const contacts = await atomo.contacts.findMany({ take: 20 })
```

- Source: `packages/atomo-client-sdk`
- Works great with React and TanStack Query.
