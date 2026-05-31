/**
 * E-commerce Schema Definition
 */

export interface Product {
  id: string;
  name: string;
  slug: string;
  description?: string;
  price: number;
  compareAtPrice?: number;
  sku?: string;
  inventory: number;
  published: boolean;
  images: string[];
  categoryId?: string;
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

export interface Order {
  id: string;
  customerId: string;
  status: OrderStatus;
  totalAmount: number;
  shippingAddress?: string;
  notes?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface OrderItem {
  id: string;
  orderId: string;
  productId: string;
  quantity: number;
  unitPrice: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface Customer {
  id: string;
  name: string;
  email: string;
  phone?: string;
  address?: string;
  createdAt: Date;
  updatedAt: Date;
}

export enum OrderStatus {
  PENDING = "pending",
  CONFIRMED = "confirmed",
  SHIPPED = "shipped",
  DELIVERED = "delivered",
  CANCELLED = "cancelled"
}

export const schema = {
  models: {
    Product: { tableName: 'products', primaryKey: 'id', searchable: ['name', 'description', 'sku'] },
    Category: { tableName: 'categories', primaryKey: 'id', searchable: ['name'] },
    Order: { tableName: 'orders', primaryKey: 'id', searchable: ['status'] },
    OrderItem: { tableName: 'order_items', primaryKey: 'id' },
    Customer: { tableName: 'customers', primaryKey: 'id', searchable: ['name', 'email'] },
  },
  config: { auditLog: true, softDeletes: true, defaultPageSize: 20 }
};

export default schema;
