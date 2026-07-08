import { model, text, select, datetime, relation, allow } from '@atomo-cc/schema'

// A minimal blog: Posts and Comments.
// Atomo generates the GraphQL API, admin UI, event sourcing,
// and migrations from this single file.

export const Post = model('Post')
  .field('title',     text().required().min(1).max(200))
  .field('content',   text())
  .field('status',    select(['draft', 'published', 'archived']).default('draft'))
  .field('publishedAt', datetime())
  .access({
    read:   allow.public(),
    create: allow.role('Admin', 'Manager'),
    update: allow.role('Admin', 'Manager'),
    delete: allow.role('Admin'),
  })

export const Comment = model('Comment')
  .field('body',       text().required().min(1).max(2000))
  .field('authorName', text().required().min(1).max(100))
  .field('postId',     relation('Post').required())
  .access({
    read:   allow.public(),
    create: allow.authenticated(),
    update: allow.role('Admin'),
    delete: allow.role('Admin'),
  })
