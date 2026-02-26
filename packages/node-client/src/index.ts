export interface ReticulumMobilePlugin {
  healthcheck(): Promise<string>;
}

export class ReticulumNodeClient {
  private readonly plugin: ReticulumMobilePlugin;

  constructor(plugin: ReticulumMobilePlugin) {
    this.plugin = plugin;
  }

  async healthcheck(): Promise<string> {
    return this.plugin.healthcheck();
  }
}
