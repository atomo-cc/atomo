import { model, text, check, index } from '@atomo/schema'

export const Note = model('notes', {
  fields: { id: text().id(), title: text() },
  constraints: [
    check("position('index(' in title) > 0"),
    index(['title']).where("title <> 'check('"),
  ],
})
