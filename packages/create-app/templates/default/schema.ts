// Atomo Schema Definition
// This file defines your content models

export interface Post {
  id: string;
  title: string;
  content: string;
  slug: string;
  published: boolean;
  publishedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}
