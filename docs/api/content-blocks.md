# Content Blocks

Atomo defines a ContentBlock GraphQL union for rich, composable content:

- ParagraphBlock
- CallLogBlock
- MeetingNoteBlock
- TaskBlock

Example query:
```graphql
query {
  contacts(limit: 1) {
    notes { 
      __typename
      ... on ParagraphBlock { content }
      ... on TaskBlock { title is_completed }
    }
  }
}
```

Blocks map to `atomo_core::ContentBlock` and are exposed in GraphQL by the server.
