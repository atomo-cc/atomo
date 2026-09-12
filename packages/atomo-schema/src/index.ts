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

// ── Model-level constraints ─────────────────────────────────────────────────

export interface ModelConstraint {
  /** `.where('predicate')` — partial unique index / partial index. */
  where(predicate: string): ModelConstraint;
}

function constraintBuilder(): ModelConstraint {
  const self: ModelConstraint = {
    where: () => self,
  };
  return self;
}

/** `unique(['tenantId', 'email'])` — composite uniqueness over field names. */
export function unique(_fields: string[]): ModelConstraint { return constraintBuilder(); }
/** `index(['tenantId', 'companyId'])` — composite secondary index. */
export function index(_fields: string[]): ModelConstraint { return constraintBuilder(); }
/** `check('amount <> 0')` — raw SQL CHECK expression over column names. */
export function check(_expr: string): ModelConstraint { return constraintBuilder(); }

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
  constraints?: ModelConstraint[];
}

export interface ModelInstance {
  readonly _table: string;
}

export function model(_table: string, _def: ModelDef): ModelInstance {
  return { _table } as ModelInstance;
}
