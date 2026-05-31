/**
 * Blog Schema Definition
 */

export interface Post {
  id: string;
  title: string;
  slug: string;
  content: string;
  excerpt?: string;
  published: boolean;
  authorId: string;
  tags: string[];
  publishedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}

export interface Author {
  id: string;
  name: string;
  email: string;
  bio?: string;
  avatarUrl?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface Category {
  id: string;
  name: string;
  slug: string;
  description?: string;
  parentId?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface Comment {
  id: string;
  postId: string;
  authorName: string;
  authorEmail: string;
  content: string;
  approved: boolean;
  createdAt: Date;
  updatedAt: Date;
}

export const schema = {
  models: {
    Post: { tableName: 'posts', primaryKey: 'id', searchable: ['title', 'content', 'slug'] },
    Author: { tableName: 'authors', primaryKey: 'id', searchable: ['name', 'email'] },
    Category: { tableName: 'categories', primaryKey: 'id', searchable: ['name'] },
    Comment: { tableName: 'comments', primaryKey: 'id', searchable: ['content'] },
  },
  config: { auditLog: true, softDeletes: true, defaultPageSize: 10 }
};

export default schema;
