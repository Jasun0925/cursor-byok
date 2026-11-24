import type { ModelTokenPricing, ModelUsage, TokenPricingSettings } from "../../../shared/api";
import { formatInteger } from "../../../shared/utils/numberFormat";
import { modelProviderName } from "../../../shared/utils/modelProvider";
import { Modal } from "../../../shared/ui/Modal";
import { VirtualList } from "../../../shared/virtual/VirtualList";
import styles from "./ModelUsageDialog.module.scss";
import { costsForUsage, pricingForModel } from "../../../shared/utils/tokenPricing";

export type UsageMetric = "calls" | "tokens" | "cache" | "cost";

export function ModelUsageDialog({ metric, rows, pricing, modelPricing, onClose }: {
  metric: UsageMetric | null;
  rows: ModelUsage[];
  pricing: TokenPricingSettings;
  modelPricing: ModelTokenPricing[];
  onClose: () => void;
}) {
  const labels = { calls: t("LLM 调用"), tokens: t("Token 消耗"), cache: t("缓存命中率"), cost: t("价值估算") };
  const title = metric ? t("{metric} · 按模型统计", { metric: labels[metric] }) : "";
  const money = (tokens: number, price: number) => `$${(tokens * price / 1_000_000).toFixed(4)}`;
  const details = (row: ModelUsage) => {
    const m = row.metrics;
    if (metric === "calls") return [
      [t("LLM 调用"), formatInteger(m.llm_calls)],
      [t("成功"), formatInteger(m.successful_calls)],
      [t("异常"), formatInteger(m.failed_calls)],
    ];
    if (metric === "cache") return [
      [t("缓存命中率"), m.cache_read_tokens + m.input_tokens > 0 ? `${(100 * m.cache_read_tokens / (m.cache_read_tokens + m.input_tokens)).toFixed(2)}%` : t("暂无数据")],
      [t("非缓存输入"), formatInteger(m.input_tokens)],
      [t("缓存读取"), formatInteger(m.cache_read_tokens)],
      [t("缓存写入"), formatInteger(m.cache_write_tokens)],
    ];
    const tokens = [m.input_tokens, m.output_tokens, m.cache_read_tokens, m.cache_write_tokens];
    const effectivePricing = pricingForModel(row.model_hash, pricing, modelPricing);
    const rates = [effectivePricing.input_per_million, effectivePricing.output_per_million, effectivePricing.cache_read_per_million, effectivePricing.cache_write_per_million];
    return [t("非缓存输入"), t("输出"), t("缓存读取"), t("缓存写入")].map((label, index) => [
      label, metric === "cost" ? money(tokens[index], rates[index]) : formatInteger(tokens[index]),
    ]);
  };
  return <Modal open={metric !== null} title={title} fullHeight scrollBody={false} closeLabel={t("关闭")} onClose={onClose}
    banner={<div className={styles.note}>{metric === "cost" ? t("按当前全局及模型级单价估算，与首页筛选条件一致。") : t("与首页当前时间和模型筛选一致。")}</div>}>
    <VirtualList items={rows} itemKey={(row) => JSON.stringify([row.model_hash, row.model_id, row.provider_type, row.provider_url])}
      className={styles.list} itemGap={12} estimatedItemHeight={130} empty={<div className={styles.empty}>{t("暂无数据")}</div>}>
      {(row) => <article className={styles.row} aria-label={row.display_name}>
        <div className={styles.heading}>
          <strong title={row.model_id}>{row.display_name}</strong>
          {metric === "tokens" && <span title={t("Token 消耗")}>{formatInteger(row.metrics.token_usage)}</span>}
          {metric === "cost" && <span title={t("价值估算")}>{money(
            Object.values(costsForUsage(row, pricing, modelPricing)).reduce((total, cost) => total + cost, 0) * 1_000_000, 1,
          )}</span>}
        </div>
        <div className={styles.provider} title={`${row.provider_url}\n${row.model_hash ?? ""}`}>{modelProviderName({ group_name: "", base_url: row.provider_url })} · {row.provider_type}</div>
        <dl className={styles.values}>{details(row).map(([label, value]) => <div key={label}>
          <dt title={label}>{label}</dt><dd title={value}>{value}</dd>
        </div>)}</dl>
      </article>}
    </VirtualList>
  </Modal>;
}
