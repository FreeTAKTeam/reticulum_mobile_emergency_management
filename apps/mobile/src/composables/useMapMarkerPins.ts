import * as maplibregl from "maplibre-gl";
import type { Map as MapLibreMap, MapMouseEvent, MapTouchEvent } from "maplibre-gl";
import type { MapMarkerRecord, MapMarkerSymbolRecord } from "@reticulum/node-client";
import { onBeforeUnmount, watch, type Ref } from "vue";

import { mdiSvgMarkup } from "../utils/mapMarkerIcons";

import { markerSymbolColor } from "../utils/mapMarkerPresentation";

const LONG_PRESS_MS = 550;
const LONG_PRESS_TOLERANCE_PX = 10;

interface MapMarkerPinOptions {
  map: Ref<MapLibreMap | null>;
  markers: Ref<MapMarkerRecord[]>;
  symbolsById: Ref<Map<string, MapMarkerSymbolRecord>>;
  visible: Ref<boolean>;
  selectedKey: Ref<string | null>;
  hiddenKey: Ref<string | null>;
  onSelect: (key: string) => void;
  onLongPress: (lat: number, lon: number) => void;
}

/** Picture markers (r3akt-uas #19) draw the relayed thumbnail itself. */
function isPicture(marker: MapMarkerRecord): boolean {
  return marker.markerType === "picture";
}

/** Thumbnail frame, or an explicit fallback when no valid thumbnail arrived. */
function renderPictureIcon(icon: Element, thumbnail: string | undefined): void {
  const frame = document.createElement("span");
  frame.className = "map-marker-picture-frame";
  if (thumbnail) {
    const image = document.createElement("img");
    image.alt = "";
    image.src = thumbnail;
    frame.append(image);
  } else {
    frame.classList.add("is-fallback");
    frame.innerHTML = mdiSvgMarkup("image");
  }
  icon.replaceChildren(frame);
}

function pinElement(onSelect: () => void, picture: boolean): HTMLDivElement {
  const element = document.createElement("div");
  element.className = picture ? "map-marker-pin is-picture" : "map-marker-pin";
  element.setAttribute("role", "button");
  element.tabIndex = 0;
  const icon = document.createElement("span");
  icon.className = "map-marker-pin-icon";
  const label = document.createElement("span");
  label.className = "map-marker-pin-label";
  element.append(icon, label);
  if (picture) {
    // The stem tip is the anchor: it sits on the marker coordinate.
    const anchor = document.createElement("span");
    anchor.className = "map-marker-picture-anchor";
    element.append(anchor);
  }
  element.addEventListener("click", (event) => {
    event.stopPropagation();
    onSelect();
  });
  element.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect();
    }
  });
  return element;
}

/** Renders shared markers as MapLibre DOM pins, separate from telemetry and SOS. */
export function useMapMarkerPins(options: MapMarkerPinOptions): void {
  const pins = new Map<string, { marker: maplibregl.Marker; element: HTMLDivElement; icon: string }>();
  let detachLongPress: (() => void) | null = null;

  function removePin(key: string): void {
    pins.get(key)?.marker.remove();
    pins.delete(key);
  }

  function sync(): void {
    const map = options.map.value;
    if (!map || !options.visible.value) {
      for (const key of [...pins.keys()]) {
        removePin(key);
      }
      return;
    }
    const active = new Set<string>();
    for (const marker of options.markers.value) {
      const key = marker.objectDestinationHash;
      if (key === options.hiddenKey.value) {
        continue;
      }
      active.add(key);
      const symbol = options.symbolsById.value.get(marker.symbol);
      let pin = pins.get(key);
      if (!pin) {
        const picture = isPicture(marker);
        const element = pinElement(() => options.onSelect(key), picture);
        pin = {
          marker: new maplibregl.Marker({ element, anchor: picture ? "bottom" : "center" })
            .setLngLat([marker.lon, marker.lat])
            .addTo(map),
          element,
          icon: "",
        };
        pins.set(key, pin);
      } else {
        pin.marker.setLngLat([marker.lon, marker.lat]);
      }
      const thumbnail = marker.picture?.thumbnailDataUrl;
      const iconKey = isPicture(marker) ? `picture:${thumbnail ?? "fallback"}` : symbol?.mdi ?? "";
      if (pin.icon !== iconKey) {
        pin.icon = iconKey;
        const icon = pin.element.querySelector(".map-marker-pin-icon");
        if (icon && isPicture(marker)) {
          renderPictureIcon(icon, thumbnail);
        } else if (icon) {
          icon.innerHTML = mdiSvgMarkup(symbol?.mdi);
        }
      }
      const label = pin.element.querySelector(".map-marker-pin-label");
      if (label) {
        label.textContent = marker.name;
      }
      pin.element.style.color = markerSymbolColor(marker.symbol);
      pin.element.dataset.symbol = marker.symbol;
      const description = isPicture(marker) && !thumbnail
        ? "Picture (no preview)"
        : symbol?.description ?? marker.symbol;
      pin.element.setAttribute("aria-label", `${description} marker: ${marker.name}`);
      pin.element.classList.toggle("is-selected", key === options.selectedKey.value);
    }
    for (const key of [...pins.keys()]) {
      if (!active.has(key)) {
        removePin(key);
      }
    }
  }

  function attachLongPress(map: MapLibreMap): () => void {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let origin: { x: number; y: number } | null = null;
    const cancel = (): void => {
      if (timer) {
        clearTimeout(timer);
      }
      timer = null;
      origin = null;
    };
    const start = (event: MapTouchEvent): void => {
      cancel();
      if (event.originalEvent.touches.length !== 1) {
        return;
      }
      origin = { x: event.point.x, y: event.point.y };
      const { lat, lng } = event.lngLat;
      timer = setTimeout(() => {
        timer = null;
        options.onLongPress(lat, lng);
      }, LONG_PRESS_MS);
    };
    const move = (event: MapTouchEvent): void => {
      if (origin && Math.hypot(event.point.x - origin.x, event.point.y - origin.y) > LONG_PRESS_TOLERANCE_PX) {
        cancel();
      }
    };
    const contextMenu = (event: MapMouseEvent): void => {
      event.preventDefault();
      options.onLongPress(event.lngLat.lat, event.lngLat.lng);
    };
    map.on("touchstart", start);
    map.on("touchmove", move);
    map.on("touchend", cancel);
    map.on("touchcancel", cancel);
    map.on("movestart", cancel);
    map.on("contextmenu", contextMenu);
    return () => {
      cancel();
      map.off("touchstart", start);
      map.off("touchmove", move);
      map.off("touchend", cancel);
      map.off("touchcancel", cancel);
      map.off("movestart", cancel);
      map.off("contextmenu", contextMenu);
    };
  }

  watch(
    options.map,
    (map) => {
      detachLongPress?.();
      detachLongPress = map ? attachLongPress(map) : null;
      sync();
    },
    { immediate: true },
  );
  watch(
    [options.markers, options.symbolsById, options.visible, options.selectedKey, options.hiddenKey],
    sync,
  );

  onBeforeUnmount(() => {
    detachLongPress?.();
    detachLongPress = null;
    for (const key of [...pins.keys()]) {
      removePin(key);
    }
  });
}
