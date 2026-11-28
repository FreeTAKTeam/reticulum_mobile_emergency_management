import { expect, test } from "vitest";
import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";

import SettingsLeaveDialog from "./SettingsLeaveDialog.vue";
import SettingsPermissionRow from "./SettingsPermissionRow.vue";
import SettingsSaveBar from "./SettingsSaveBar.vue";

const render = (component: Component, props: Record<string, unknown>) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));

test("save bar reflects clean, dirty, saving and feedback states", async () => {
  const clean = await render(SettingsSaveBar, { dirty: false, saving: false, feedback: "" });
  expect(clean).toContain("All changes saved");
  expect(clean.match(/disabled/g)).toHaveLength(2);

  const dirty = await render(SettingsSaveBar, { dirty: true, saving: false, feedback: "" });
  expect(dirty).toContain("Unsaved changes");
  expect(dirty).not.toContain("disabled");

  const saving = await render(SettingsSaveBar, { dirty: true, saving: true, feedback: "" });
  expect(saving).toContain("Saving");

  const feedback = await render(SettingsSaveBar, { dirty: false, saving: false, feedback: "Saved." });
  expect(feedback).toContain('role="status"');
  expect(feedback).toContain("Saved.");
});

test("leave dialog renders only when open, as an alert dialog", async () => {
  expect(await render(SettingsLeaveDialog, { open: false })).not.toContain("alertdialog");
  const html = await render(SettingsLeaveDialog, { open: true });
  expect(html).toContain('role="alertdialog"');
  expect(html).toContain("Keep editing");
  expect(html).toContain("Discard");
});

test("permission row starts in the checking state", async () => {
  const never = () => new Promise<never>(() => undefined);
  const html = await render(SettingsPermissionRow, { label: "Location", check: never, request: never });
  expect(html).toContain("<dt>Location</dt>");
  expect(html).toContain("Checking…");
  expect(html).not.toContain("Request");
});
