/**
 * @atomo-cc/schema — builder-pattern DSL for defining Atomo schemas.
 *
 * These functions exist for TypeScript type-checking and IDE support.
 * The Rust parser reads the source code directly; at runtime these
 * builders are only used by tooling (codegen CLI, schema validator).
 */

// ── Field builders ──────────────────────────────────────────────────────────

export interface FieldBuilder<T = unknown> {
  id(): FieldBuilder<T>;
  required(): FieldBuilder<T>;
  optional(): FieldBuilder<T>;
  min(n: number): FieldBuilder<T>;
  max(n: number): FieldBuilder<T>;
  default(value: T): FieldBuilder<T>;
  defaultNow(): FieldBuilder<T>;
  autoUpdate(): FieldBuilder<T>;
  lowercase(): FieldBuilder<T>;
  trim(): FieldBuilder<T>;
  unique(): FieldBuilder<T>;
  index(): FieldBuilder<T>;
}

function fieldBuilder<T = unknown>(): FieldBuilder<T> {
  const self: FieldBuilder<T> = {
    id: () => self,
    required: () => self,
    optional: () => self,
    min: () => self,
    max: () => self,
    default: () => self,
    defaultNow: () => self,
    autoUpdate: () => self,
    lowercase: () => self,
    trim: () => self,
    unique: () => self,
    index: () => self,
  };
  return self;
}

export function text(): FieldBuilder<string> { return fieldBuilder(); }
export function email(): FieldBuilder<string> { return fieldBuilder(); }
export function url(): FieldBuilder<string> { return fieldBuilder(); }
export function number(): FieldBuilder<number> { return fieldBuilder(); }
export function boolean(): FieldBuilder<boolean> { return fieldBuilder(); }
export function datetime(): FieldBuilder<string> { return fieldBuilder(); }
export function json(): FieldBuilder<unknown> { return fieldBuilder(); }
export function file(): FieldBuilder<string> { return fieldBuilder(); }
export function select<T extends string>(_options: T[]): FieldBuilder<T> { return fieldBuilder(); }
export function relation(_target: string): FieldBuilder<string> { return fieldBuilder(); }

// ── Access control ──────────────────────────────────────────────────────────

export interface AccessRule { readonly _brand: unique symbol }

export const allow = {
  role(roles: string | string[]): AccessRule {
    return { _brand: Symbol() } as unknown as AccessRule;
  },
  authenticated(): AccessRule {
    return { _brand: Symbol() } as unknown as AccessRule;
  },
  public(): AccessRule {
    return { _brand: Symbol() } as unknown as AccessRule;
  },
};

// ── Action builder ──────────────────────────────────────────────────────────

export interface ActionRef {
  whenChanged(field: string): ActionRef;
}

export interface ActionBuilder extends ActionRef {
  from(table: string): ActionBuilder;
  input(fields: string[]): ActionBuilder;
}

export function action(_name: string): ActionBuilder {
  const self: ActionBuilder = {
    from: () => self,
    input: () => self,
    whenChanged: () => self,
  };
  return self;
}

// ── Model builder ───────────────────────────────────────────────────────────

export interface ModelDef {
  fields: Record<string, FieldBuilder>;
  access?: {
    create?: AccessRule;
    read?: AccessRule;
    update?: AccessRule;
    delete?: AccessRule;
  };
  on?: {
    created?: ActionRef[];
    updated?: ActionRef[];
    deleted?: ActionRef[];
  };
}

export interface ModelInstance {
  readonly _table: string;
}

export function model(_table: string, _def: ModelDef): ModelInstance {
  return { _table } as ModelInstance;
}
