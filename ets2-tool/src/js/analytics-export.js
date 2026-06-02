import { invoke } from "../modules/shared/runtime.js";

export async function exportAnalyticsCsv(filters = {}) {
  return invoke("career_export_analytics_csv", {
    filters,
  });
}
