// Atomo Schema Definition
// This file defines your content models and will be used to generate 
// Rust backend code and TypeScript frontend types

export interface Contact {
  id: string;
  firstName: string;
  lastName: string;
  email: string;
  phone?: string;
  companyId?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface Company {
  id: string;
  name: string;
  address?: string;
  website?: string;
  createdAt: Date;
  updatedAt: Date;
}
