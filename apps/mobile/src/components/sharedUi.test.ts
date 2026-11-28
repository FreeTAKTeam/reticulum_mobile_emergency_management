import { expect, test } from "vitest";
import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";

import ListWindowControls from "./ListWindowControls.vue";
import StatusPill from "./StatusPill.vue";

const render = (component: Component, props: Record<string, unknown>) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));

test("status pill shows the value, its meaning and a colour class", async () => {
  const html = await render(StatusPill, { label: "Security", value: "Red" });
  expect(html).toContain("pill with-label red");
  expect(html).toContain('title="Security: Red (Critical)"');
  expect(html).toContain("Red status: Critical");
});

test("status pill without a label uses the compact class", async () => {
  const html = await render(StatusPill, { label: "  ", value: "Unknown" });
  expect(html).toContain("pill without-label unknown");
  expect(html).not.toContain("pill-label");
});

test("list window controls appear only for lists over 200 items", async () => {
  const props = { start: 0, end: 200, hasPrevious: false, hasNext: true };
  expect(await render(ListWindowControls, { ...props, total: 200 })).not.toContain("<nav");
  const html = await render(ListWindowControls, { ...props, total: 450, nextLabel: "Older" });
  expect(html).toContain("1-200 of 450");
  expect(html).toContain("Older");
  expect(html).toMatch(/<button type="button" disabled[^>]*>\s*Previous/);
});
