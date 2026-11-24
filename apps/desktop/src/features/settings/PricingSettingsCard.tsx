import { useEffect, useState } from "react";
import type { Model, ModelTokenPricing, TokenPricingSettings } from "../../shared/api";
import { appStore, DEFAULT_TOKEN_PRICING, useAppStore } from "../../shared/store/appStore";
import { Button } from "../../shared/ui/Button";
import { FormField, TextInput } from "../../shared/ui/FormControls";
import { TitledCard } from "../../shared/ui/TitledCard";
import { useMessage } from "../../shared/ui/message";
import styles from "./PricingSettingsCard.module.scss";

type PricingDraft = {
  input_per_million: string;
  output_per_million: string;
  cache_read_per_million: string;
  cache_write_per_million: string;
};

type ModelPricingRow = {
  model_hash: string;
  display_name: string;
  model_id: string;
};

type ModelPricingState = {
  rows: ModelPricingRow[];
  drafts: Record<string, PricingDraft>;
};

function toDraft(pricing: TokenPricingSettings): PricingDraft {
  return {
    input_per_million: String(pricing.input_per_million),
    output_per_million: String(pricing.output_per_million),
    cache_read_per_million: String(pricing.cache_read_per_million),
    cache_write_per_million: String(pricing.cache_write_per_million),
  };
}

function formatPrice(value: number) {
  return `$${value}`;
}

function parsePrice(value: string, label: string) {
  const price = Number(value);
  if (!Number.isFinite(price) || price < 0) {
    throw new Error(t("{label}必须是非负数", { label }));
  }
  return price;
}

function toSettings(draft: PricingDraft): TokenPricingSettings {
  return {
    input_per_million: parsePrice(draft.input_per_million, t("输入价格")),
    output_per_million: parsePrice(draft.output_per_million, t("输出价格")),
    cache_read_per_million: parsePrice(draft.cache_read_per_million, t("缓存读取价格")),
    cache_write_per_million: parsePrice(draft.cache_write_per_million, t("缓存写入价格")),
  };
}

function samePricing(left: TokenPricingSettings, right: TokenPricingSettings) {
  return left.input_per_million === right.input_per_million
    && left.output_per_million === right.output_per_million
    && left.cache_read_per_million === right.cache_read_per_million
    && left.cache_write_per_million === right.cache_write_per_million;
}

function modelPricingState(global: TokenPricingSettings, saved: ModelTokenPricing[], models: Model[]): ModelPricingState {
  const savedByHash = new Map(saved.map((entry) => [entry.model_hash, entry]));
  const rows = models.map((model) => ({
    model_hash: model.model_hash,
    display_name: model.display_name,
    model_id: model.model_id,
  }));
  const known = new Set(rows.map((row) => row.model_hash));
  for (const entry of saved) {
    if (!known.has(entry.model_hash)) {
      rows.push({ model_hash: entry.model_hash, display_name: entry.model_hash, model_id: "" });
    }
  }
  return {
    rows,
    drafts: Object.fromEntries(rows.map((row) => [
      row.model_hash,
      toDraft(savedByHash.get(row.model_hash) ?? global),
    ])),
  };
}

export function PricingSettingsCard() {
  const { pricing, modelPricing, models } = useAppStore();
  const message = useMessage();
  const [draft, setDraft] = useState<PricingDraft>(() => toDraft(pricing));
  const [modelDrafts, setModelDrafts] = useState<Record<string, PricingDraft>>(
    () => modelPricingState(pricing, modelPricing, models).drafts,
  );
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const modelRows = modelPricingState(pricing, modelPricing, models).rows;

  useEffect(() => {
    if (!editing) {
      setDraft(toDraft(pricing));
      setModelDrafts(modelPricingState(pricing, modelPricing, models).drafts);
    }
  }, [pricing, modelPricing, models, editing]);

  const edit = () => {
    setDraft(toDraft(pricing));
    setModelDrafts(modelPricingState(pricing, modelPricing, models).drafts);
    setEditing(true);
  };

  const cancel = () => {
    setDraft(toDraft(pricing));
    setModelDrafts(modelPricingState(pricing, modelPricing, models).drafts);
    setEditing(false);
  };

  const save = async () => {
    try {
      setSaving(true);
      const next = toSettings(draft);
      const modelOverrides = modelRows
        .map((row) => ({ model_hash: row.model_hash, ...toSettings(modelDrafts[row.model_hash] ?? toDraft(next)) }))
        .filter((entry) => !samePricing(entry, next));
      if (!await appStore.updatePricingSettings(next)) return;
      if (!await appStore.updateModelPricingSettings(modelOverrides)) return;
      setEditing(false);
      message(t("定价设置已保存"));
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };

  const restoreDefault = () => {
    setDraft(toDraft(DEFAULT_TOKEN_PRICING));
  };

  const action = editing ? (
    <div className={styles.actionGroup}>
      <Button size="small" disabled={saving} onClick={restoreDefault}>{t("恢复默认")}</Button>
      <Button size="small" disabled={saving} onClick={cancel}>{t("取消")}</Button>
      <Button variant="primary" size="small" disabled={saving} onClick={() => void save()}>
        {saving ? t("保存中…") : t("保存")}
      </Button>
    </div>
  ) : (
    <button type="button" className={styles.headerAction} onClick={edit}>{t("编辑")}</button>
  );

  return (
    <TitledCard title={t("Token 定价")} action={action}>
      <div className={styles.content}>
        <small>{t("用于首页价值估算的 Token 单价，单位：美元 / 百万 Token。")}</small>
        {editing ? (
          <>
            <div className={styles.customFields}>
              <FormField label={t("输入价格（$/1M）")}>
                <TextInput
                  type="number"
                  min={0}
                  step="any"
                  value={draft.input_per_million}
                  onChange={(event) => setDraft({ ...draft, input_per_million: event.target.value })}
                />
              </FormField>
              <FormField label={t("输出价格（$/1M）")}>
                <TextInput
                  type="number"
                  min={0}
                  step="any"
                  value={draft.output_per_million}
                  onChange={(event) => setDraft({ ...draft, output_per_million: event.target.value })}
                />
              </FormField>
              <FormField label={t("缓存读取价格（$/1M）")}>
                <TextInput
                  type="number"
                  min={0}
                  step="any"
                  value={draft.cache_read_per_million}
                  onChange={(event) => setDraft({ ...draft, cache_read_per_million: event.target.value })}
                />
              </FormField>
              <FormField label={t("缓存写入价格（$/1M）")}>
                <TextInput
                  type="number"
                  min={0}
                  step="any"
                  value={draft.cache_write_per_million}
                  onChange={(event) => setDraft({ ...draft, cache_write_per_million: event.target.value })}
                />
              </FormField>
            </div>
            <div className={styles.modelSection}>
              <div className={styles.modelHeader}>
                <strong>{t("模型级覆盖")}</strong>
                <small>{t("未设置覆盖的模型使用全局单价；插件模型暂不提供单独定价。")}</small>
              </div>
              {modelRows.length === 0 ? <small>{t("暂无已配置模型")}</small> : modelRows.map((row) => {
                const rowDraft = modelDrafts[row.model_hash] ?? draft;
                return <div className={styles.modelRow} key={row.model_hash}>
                  <div className={styles.modelIdentity}>
                    <strong title={row.model_id}>{row.display_name}</strong>
                    <code>{row.model_hash}</code>
                  </div>
                  <div className={styles.modelFields}>
                    {[
                      ["input_per_million", t("输入")],
                      ["output_per_million", t("输出")],
                      ["cache_read_per_million", t("缓存读取")],
                      ["cache_write_per_million", t("缓存写入")],
                    ].map(([key, label]) => {
                      const field = key as keyof PricingDraft;
                      return <label className={styles.modelField} key={key}>
                        <span>{label}</span>
                        <TextInput
                          type="number"
                          min={0}
                          step="any"
                          value={rowDraft[field]}
                          onChange={(event) => setModelDrafts({
                            ...modelDrafts,
                            [row.model_hash]: { ...rowDraft, [field]: event.target.value },
                          })}
                        />
                      </label>;
                    })}
                  </div>
                </div>;
              })}
            </div>
          </>
        ) : (
          <>
            <div className={styles.row}>
              <strong>{t("输入价格（$/1M）")}</strong>
              <span className={styles.value}>{formatPrice(pricing.input_per_million)}</span>
            </div>
            <div className={styles.row}>
              <strong>{t("输出价格（$/1M）")}</strong>
              <span className={styles.value}>{formatPrice(pricing.output_per_million)}</span>
            </div>
            <div className={styles.row}>
              <strong>{t("缓存读取价格（$/1M）")}</strong>
              <span className={styles.value}>{formatPrice(pricing.cache_read_per_million)}</span>
            </div>
            <div className={styles.row}>
              <strong>{t("缓存写入价格（$/1M）")}</strong>
              <span className={styles.value}>{formatPrice(pricing.cache_write_per_million)}</span>
            </div>
            <div className={styles.modelSection}>
              <div className={styles.modelHeader}>
                <strong>{t("模型级覆盖")}</strong>
                <small>{t("未设置覆盖的模型使用全局单价；插件模型暂不提供单独定价。")}</small>
              </div>
              {modelPricing.length === 0 ? <small>{t("暂无模型级覆盖")}</small> : modelPricing.map((entry) => {
                const row = modelRows.find((candidate) => candidate.model_hash === entry.model_hash);
                return <div className={styles.row} key={entry.model_hash}>
                  <strong title={row?.model_id}>{row?.display_name ?? entry.model_hash}</strong>
                  <span className={styles.value} title={entry.model_hash}>
                    ${entry.input_per_million} / ${entry.output_per_million} / ${entry.cache_read_per_million} / ${entry.cache_write_per_million}
                  </span>
                </div>;
              })}
            </div>
          </>
        )}
      </div>
    </TitledCard>
  );
}
