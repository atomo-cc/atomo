import { useQuery } from "@tanstack/react-query";
import { apiClient } from "./api";
import { summarizeAuditPolicy } from "./audit-policy";

export function useAuditPolicy() {
  const query = useQuery({
    queryKey: ["storage-audit-policy"],
    queryFn: () => apiClient.getAuditPolicy(),
    staleTime: 60_000,
    retry: false,
  });
  // Never retain an earlier policy claim after a failed/forbidden refresh.
  return summarizeAuditPolicy(query.isError ? undefined : query.data);
}
