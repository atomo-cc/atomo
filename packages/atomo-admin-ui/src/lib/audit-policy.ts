export type AuditMode = "full" | "metadata" | "off";
export interface AuditPolicy {
  mode: AuditMode;
  max_age_secs?: number | null;
  max_bytes?: number | null;
}
export interface AuditConfiguration {
  default: AuditPolicy;
  models: Record<string, AuditPolicy>;
}

/** Describe configured saving, not a claim that historical audit delivery is complete. */
export function summarizeAuditPolicy(config?: AuditConfiguration): {
  label: string;
  description: string;
  empty: string;
} {
  if (!config)
    return {
      label: "Unknown",
      description:
        "Effective audit policy unavailable. Administrator access is required.",
      empty: "No audit entries available. Effective saving policy is unknown.",
    };
  const policies = [config.default, ...Object.values(config.models)];
  const modes = new Set(policies.map((policy) => policy.mode));
  const limited = policies.some(
    (policy) => policy.max_age_secs != null || policy.max_bytes != null,
  );
  const suffix = limited
    ? " Configured retention can remove older entries."
    : "";
  if (modes.size > 1)
    return {
      label: "Per-model policy",
      description:
        "Audit saving varies by model: full details, metadata only, or off." +
        suffix,
      empty: "No audit entries available. Saving varies by model." + suffix,
    };
  switch (config.default.mode) {
    case "off":
      return {
        label: "Off",
        description:
          "New audit entries are not saved. Existing entries may remain." +
          suffix,
        empty: "No audit entries available. New audit saving is off." + suffix,
      };
    case "metadata":
      return {
        label: "Metadata only",
        description:
          "Operation, entity, actor and time are saved without payload, IP or user agent." +
          suffix,
        empty:
          "No audit entries available. Metadata-only saving is configured." +
          suffix,
      };
    case "full":
      return {
        label: "Full details",
        description:
          "Full audit details are configured; event delivery is asynchronous." +
          suffix,
        empty: "No audit entries available." + suffix,
      };
    default:
      return summarizeAuditPolicy();
  }
}
