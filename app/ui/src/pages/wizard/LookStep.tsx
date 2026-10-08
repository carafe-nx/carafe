import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { IconCropper } from "@/features/icon-crop/IconCropper";
import { renderIcon } from "@/features/icon-render/renderIcon";
import { usePreferences } from "@/features/preferences/usePreferences";
import { ArtPicker } from "@/features/steamgriddb/ArtPicker";
import type { Crop } from "@/shared/api/bindings/Crop";
import type { IconSource } from "@/shared/api/bindings/IconSource";
import type { Metadata } from "@/shared/api/bindings/Metadata";
import { api, isCommandError } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/Field";
import { Notice } from "@/shared/ui/Notice";
import { Segmented } from "@/shared/ui/Segmented";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./WizardPage.module.css";
import { iconChosen } from "./wizardState";

const NO_CROP: Crop = { x: 0, y: 0, size: 0 };
const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"];
const PREVIEW_DELAY_MS = 250;

type LookStepProps = {
  metadata: Metadata;
  buildNumber: number;
  folder: string | null;
  executable: string | null;
  company: string | null;
  onChange: (metadata: Metadata) => void;
};

function sourceKey(icon: IconSource): string {
  switch (icon.kind) {
    case "executable":
      return "executable";
    case "file":
      return `file:${icon.path}`;
    case "steamGridDb":
      return `steamGridDb:${icon.url}`;
  }
}

function loadSource(key: string): Promise<string> | null {
  const [kind, ...rest] = key.split(":");
  const value = rest.join(":");
  if (value === "") {
    return null;
  }
  if (kind === "file") {
    return api.readImage(value);
  }
  if (kind === "steamGridDb") {
    return api.artDownload(value);
  }
  return null;
}

function sameCrop(a: Crop, b: Crop): boolean {
  return a.x === b.x && a.y === b.y && a.size === b.size;
}

export function LookStep({ metadata, buildNumber, folder, executable, company, onChange }: LookStepProps) {
  const { t } = useTranslation();
  const { preferences, save } = usePreferences();
  const icon = metadata.icon;
  const key = sourceKey(icon);
  const [source, setSource] = useState<{ key: string; url: string } | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [exePreview, setExePreview] = useState<{ url: string; missing: boolean } | null>(null);

  useEffect(() => {
    const load = loadSource(key);
    if (!load) {
      return;
    }
    let cancelled = false;
    setSourceError(null);
    load
      .then((url) => {
        if (!cancelled) {
          setSource({ key, url });
        }
      })
      .catch((failure: unknown) => {
        if (!cancelled) {
          setSourceError(isCommandError(failure) ? failure.code : "unknown");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [key]);

  useEffect(() => {
    if (icon.kind !== "executable") {
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      renderIcon(metadata, folder, executable)
        .then((rendered) => {
          if (!cancelled) {
            setExePreview({ url: rendered.preview, missing: rendered.missingExeIcon });
          }
        })
        .catch(() => {
          if (!cancelled) {
            setExePreview(null);
          }
        });
    }, PREVIEW_DELAY_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [icon.kind, metadata, folder, executable]);

  const onCrop = useCallback(
    (crop: Crop) => {
      if ((icon.kind === "file" || icon.kind === "steamGridDb") && !sameCrop(icon.crop, crop)) {
        onChange({ ...metadata, icon: { ...icon, crop } });
      }
    },
    [icon, metadata, onChange],
  );

  const pickFile = async () => {
    const path = await open({ multiple: false, filters: [{ name: t("look.imageFiles"), extensions: IMAGE_EXTENSIONS }] });
    if (typeof path === "string") {
      onChange({ ...metadata, icon: { kind: "file", path, crop: NO_CROP } });
    }
  };

  const chooseKind = (kind: IconSource["kind"]) => {
    if (kind === icon.kind) {
      return;
    }
    if (kind === "executable") {
      onChange({ ...metadata, icon: { kind } });
    } else if (kind === "file") {
      onChange({ ...metadata, icon: { kind, path: "", crop: NO_CROP } });
    } else {
      onChange({ ...metadata, icon: { kind, gameId: 0, imageId: 0, url: "", crop: NO_CROP } });
    }
  };

  const croppable = icon.kind !== "executable" && source?.key === key ? source.url : null;
  const initialCrop = icon.kind === "executable" ? null : icon.crop;
  const suggestedPublisher = company && company !== metadata.publisher ? company : null;
  const publisherHint = (
    <>
      {t("look.publisherHint")}
      {suggestedPublisher ? (
        <>
          {" "}
          {t("look.publisherInExe", { company: suggestedPublisher })}{" "}
          <button type="button" className={styles.hintAction} onClick={() => onChange({ ...metadata, publisher: suggestedPublisher })}>
            {t("look.publisherUse")}
          </button>
        </>
      ) : null}
    </>
  );

  return (
    <div className={styles.lookGrid}>
      <div className={styles.stepBody}>
        <Field label={t("look.title")} htmlFor="title">
          <TextInput id="title" value={metadata.title} onChange={(event) => onChange({ ...metadata, title: event.target.value })} />
        </Field>
        <Field label={t("look.publisher")} htmlFor="publisher" hint={publisherHint}>
          <TextInput id="publisher" value={metadata.publisher} onChange={(event) => onChange({ ...metadata, publisher: event.target.value })} />
        </Field>
        <Field label={t("look.version")} htmlFor="version" hint={t("look.versionHint", { version: `1.${buildNumber}` })}>
          <TextInput
            id="version"
            value={metadata.displayVersion ?? ""}
            placeholder={`1.${buildNumber}`}
            onChange={(event) => onChange({ ...metadata, displayVersion: event.target.value === "" ? null : event.target.value })}
          />
        </Field>
        <Field label={t("look.icon")}>
          <Segmented<IconSource["kind"]>
            label={t("look.icon")}
            value={icon.kind}
            options={[
              { value: "executable", label: t("look.iconExe") },
              { value: "file", label: t("look.iconFile") },
              { value: "steamGridDb", label: t("look.iconSteamGridDb") },
            ]}
            onChange={chooseKind}
          />
        </Field>
        {icon.kind === "file" ? (
          <div className={styles.pathRow}>
            <TextInput value={icon.path} readOnly placeholder={t("look.noFile")} aria-label={t("look.iconFile")} />
            <Button onClick={() => void pickFile()}>{t("common.browse")}</Button>
          </div>
        ) : null}
        {icon.kind === "steamGridDb" ? (
          <ArtPicker
            initialTerm={metadata.title}
            selectedImageId={icon.imageId || null}
            hasKey={(preferences.steamGridDbKey ?? "").trim() !== ""}
            onSaveKey={(steamGridDbKey) => save({ ...preferences, steamGridDbKey })}
            onPick={(gameId, image) =>
              onChange({ ...metadata, icon: { kind: "steamGridDb", gameId, imageId: image.id, url: image.url, crop: NO_CROP } })
            }
          />
        ) : null}
        {sourceError && icon.kind !== "executable" ? <Notice tone="error">{t(`errors.${sourceError}`)}</Notice> : null}
        {iconChosen(icon) ? null : <Notice tone="info">{t("look.iconRequired")}</Notice>}
      </div>
      <div className={styles.preview}>
        <span className={styles.previewLabel}>{t("look.preview")}</span>
        {icon.kind === "executable" ? (
          <div className={styles.previewTile}>
            {exePreview ? <img className={styles.iconPreview} src={exePreview.url} alt="" /> : null}
            <span className={styles.muted}>{exePreview?.missing ? t("look.exeNoIcon") : t("look.exePreviewHint")}</span>
          </div>
        ) : croppable ? (
          <IconCropper key={key} src={croppable} initial={initialCrop} onChange={onCrop} />
        ) : (
          <div className={styles.previewTile}>
            <span className={styles.muted}>{icon.kind === "file" ? t("look.pickImage") : t("look.pickArt")}</span>
          </div>
        )}
      </div>
    </div>
  );
}
