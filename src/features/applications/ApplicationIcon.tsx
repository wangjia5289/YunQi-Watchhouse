import { CSSProperties, useEffect, useRef, useState } from "react";
import { getApplicationIcon } from "../../lib/ipc";

interface CachedIcon {
  url: string | null;
  revision: string | null;
  checkedAt: number;
}

const REVALIDATE_AFTER_MS = 60_000;
const iconCache = new Map<number, CachedIcon>();
const pendingRequests = new Map<number, Promise<CachedIcon>>();

export function clearApplicationIconMemoryCache() {
  for (const icon of iconCache.values()) {
    if (icon.url) URL.revokeObjectURL(icon.url);
  }
  iconCache.clear();
  pendingRequests.clear();
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0])
    .join("")
    .toUpperCase();
}

function loadIcon(applicationId: number): Promise<CachedIcon> {
  const pending = pendingRequests.get(applicationId);
  if (pending) return pending;

  const request = getApplicationIcon(applicationId)
    .then((icon) => {
      const previous = iconCache.get(applicationId);
      if (!icon) {
        const next = { url: null, revision: null, checkedAt: Date.now() };
        iconCache.set(applicationId, next);
        return next;
      }
      if (previous?.revision === icon.revision && previous.url) {
        const next = { ...previous, checkedAt: Date.now() };
        iconCache.set(applicationId, next);
        return next;
      }
      const url = URL.createObjectURL(
        new Blob([new Uint8Array(icon.bytes)], { type: icon.mimeType }),
      );
      if (previous?.url) URL.revokeObjectURL(previous.url);
      const next = { url, revision: icon.revision, checkedAt: Date.now() };
      iconCache.set(applicationId, next);
      return next;
    })
    .catch(() => {
      const previous = iconCache.get(applicationId);
      return previous ?? { url: null, revision: null, checkedAt: Date.now() };
    })
    .finally(() => pendingRequests.delete(applicationId));
  pendingRequests.set(applicationId, request);
  return request;
}

export function ApplicationIcon({
  applicationId,
  applicationName,
  className = "application-icon",
  style,
}: {
  applicationId: number;
  applicationName: string;
  className?: string;
  style?: CSSProperties;
}) {
  const elementRef = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(
    () => iconCache.has(applicationId) || typeof IntersectionObserver === "undefined",
  );
  const [iconUrl, setIconUrl] = useState<string | null>(
    () => iconCache.get(applicationId)?.url ?? null,
  );

  useEffect(() => {
    if (visible) return;
    const element = elementRef.current;
    if (!element) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "240px 0px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [visible]);

  useEffect(() => {
    if (!visible) return;
    let active = true;
    const cached = iconCache.get(applicationId);
    if (cached && Date.now() - cached.checkedAt < REVALIDATE_AFTER_MS) {
      setIconUrl(cached.url);
    } else {
      void loadIcon(applicationId).then((icon) => {
        if (active) setIconUrl(icon.url);
      });
    }
    return () => {
      active = false;
    };
  }, [applicationId, visible]);

  return (
    <span ref={elementRef} className={`${className}${iconUrl ? " has-image" : ""}`} style={style}>
      {iconUrl ? <img src={iconUrl} alt="" /> : initials(applicationName)}
    </span>
  );
}
