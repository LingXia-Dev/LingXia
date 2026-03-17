const CLOUD_PAGE_TYPES = {
  AUTH: "auth",
  MQTT: "mqtt",
} as const;

const MQTT_SHORT_TOPIC = "demo/mqtt";
type CloudPageType =
  (typeof CLOUD_PAGE_TYPES)[keyof typeof CLOUD_PAGE_TYPES];

type TenantLike = {
  tenantId?: string;
  tenantName?: string;
  logoUrl?: string;
};

function getErrorMessage(error: unknown, fallback: string): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  return fallback;
}

function normalizePageType(raw?: string): CloudPageType {
  return raw === CLOUD_PAGE_TYPES.MQTT
    ? CLOUD_PAGE_TYPES.MQTT
    : CLOUD_PAGE_TYPES.AUTH;
}

function getNavigationTitle(pageType: CloudPageType): string {
  return pageType === CLOUD_PAGE_TYPES.MQTT
    ? "Cloud MQTT Demo"
    : "Cloud Auth Demo";
}

function formatMqttPayload(payload: unknown): string {
  if (typeof payload === "string") {
    return payload;
  }
  if (payload === null || payload === undefined) {
    return "";
  }
  try {
    return JSON.stringify(payload, null, 2);
  } catch (_error) {
    return String(payload);
  }
}

function normalizeTenant(tenant: any): TenantLike | null {
  if (!tenant || typeof tenant !== "object") {
    return null;
  }
  return {
    tenantId:
      typeof tenant.tenantId === "string" ? tenant.tenantId : "",
    tenantName:
      typeof tenant.tenantName === "string" ? tenant.tenantName : "",
    logoUrl:
      typeof tenant.logoUrl === "string" ? tenant.logoUrl : "",
  };
}

function normalizeTenants(tenants: any[]): TenantLike[] {
  return tenants
    .map((tenant) => normalizeTenant(tenant))
    .filter((tenant): tenant is TenantLike => tenant !== null);
}

Page({
  mqttSubscription: null as any,
  mqttLoopId: 0,

  data: {
    type: CLOUD_PAGE_TYPES.AUTH,
    status: "Idle",
    tenant: null,
    tenants: [],
    accessToken: "",
    mqttStatus: "Idle",
    mqttSubscribed: false,
    mqttTopicFilter: MQTT_SHORT_TOPIC,
    mqttMessageCount: 0,
    mqttLastTopic: "",
    mqttLastPayload: "",
    mqttLastReceivedAt: "",
  },

  onLoad: async function (options = {}) {
    const { type } = options as { type?: string };
    const pageType = normalizePageType(type);
    this.setData({
      type: pageType,
    });
    lx.setNavigationBarTitle({
      title: getNavigationTitle(pageType),
    });
    if (pageType === CLOUD_PAGE_TYPES.MQTT) {
      await this.startMqttDemo();
      return;
    }
    await this.refreshSnapshot();
  },

  onShow: async function () {
    lx.setNavigationBarTitle({
      title: getNavigationTitle(this.data.type),
    });
    if (this.data.type === CLOUD_PAGE_TYPES.MQTT) {
      await this.startMqttDemo();
      return;
    }
    await this.refreshSnapshot();
  },

  onUnload: async function () {
    await this.stopMqttDemo();
  },

  refreshSnapshot: async function () {
    try {
      const tenants = await lx.auth.getTenants();
      const currentTenant = normalizeTenant(lx.auth.tenant);
      this.setData({
        status: "Ready",
        tenant: currentTenant,
        tenants: normalizeTenants(tenants),
      });
    } catch (error) {
      const message = getErrorMessage(error, "Load cloud state failed");
      this.setData({
        status: message,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  loginInteractive: async function () {
    this.setData({
      status: "Starting interactive login...",
    });
    try {
      await lx.auth.login();
      this.setData({
        accessToken: "",
        status: "Login succeeded",
      });
      await this.refreshSnapshot();
    } catch (error) {
      const message = getErrorMessage(error, "Interactive login failed");
      this.setData({
        status: message,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  getAccessToken: async function () {
    this.setData({
      status: "Fetching access token...",
    });
    try {
      const accessToken = await lx.auth.getAccessToken();
      this.setData({
        accessToken,
        status: "Access token ready",
      });
      await this.refreshSnapshot();
    } catch (error) {
      const message = getErrorMessage(error, "Get access token failed");
      this.setData({
        status: message,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  logoutCurrentTenant: async function () {
    this.setData({
      status: "Logging out...",
    });
    try {
      await lx.auth.logout();
      this.setData({
        accessToken: "",
        status: "Logged out",
      });
      await this.refreshSnapshot();
    } catch (error) {
      const message = getErrorMessage(error, "Logout failed");
      this.setData({
        status: message,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  switchTenant: async function (params = {}) {
    const { tenantId } = params as { tenantId?: string };
    if (!tenantId) {
      return;
    }
    this.setData({
      status: `Switching to ${tenantId}...`,
    });
    try {
      await lx.auth.switchTenant(tenantId);
      this.setData({
        accessToken: "",
        status: `Switched to ${tenantId}`,
      });
      await this.refreshSnapshot();
    } catch (error) {
      const message = getErrorMessage(error, "Switch tenant failed");
      this.setData({
        status: message,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  startMqttDemo: async function () {
    if (this.mqttSubscription) {
      return;
    }
    this.setData({
      mqttStatus: `Subscribing to ${MQTT_SHORT_TOPIC}...`,
    });
    try {
      const subscription = await lx.cloud.mqtt.subscribe(MQTT_SHORT_TOPIC, {
        parse: "auto",
      });
      this.mqttSubscription = subscription;
      this.mqttLoopId += 1;
      const loopId = this.mqttLoopId;
      this.setData({
        mqttStatus: `Subscribed to ${MQTT_SHORT_TOPIC}`,
        mqttSubscribed: true,
      });
      void this.consumeMqtt(subscription, loopId);
    } catch (error) {
      const message = getErrorMessage(error, "MQTT subscribe failed");
      this.setData({
        mqttStatus: message,
        mqttSubscribed: false,
      });
      lx.showToast({ title: message, icon: "none" });
    }
  },

  stopMqttDemo: async function () {
    const subscription = this.mqttSubscription;
    this.mqttSubscription = null;
    this.mqttLoopId += 1;
    if (!subscription) {
      this.setData({
        mqttStatus: "Not subscribed",
        mqttSubscribed: false,
      });
      return;
    }
    try {
      await subscription.unsubscribe();
      this.setData({
        mqttStatus: "Subscription stopped",
        mqttSubscribed: false,
      });
    } catch (_error) {
      // Ignore duplicate cleanup while leaving the page.
    }
  },

  consumeMqtt: async function (
    subscription: any,
    loopId: number
  ) {
    try {
      for await (const message of subscription) {
        if (loopId !== this.mqttLoopId) {
          break;
        }
        console.log("[cloud][mqtt] received", {
          topic: message.topic,
          payload: message.payload,
          qos: message.qos,
          receivedAt: message.receivedAt,
        });
        const nextCount = this.data.mqttMessageCount + 1;
        this.setData({
          mqttStatus: `Received message #${nextCount}`,
          mqttMessageCount: nextCount,
          mqttLastTopic: message.topic,
          mqttLastPayload: formatMqttPayload(message.payload),
          mqttLastReceivedAt: new Date(message.receivedAt).toLocaleString(),
        });
      }
    } catch (error) {
      if (loopId !== this.mqttLoopId) {
        return;
      }
      const message = getErrorMessage(error, "MQTT stream failed");
      this.setData({
        mqttStatus: message,
        mqttSubscribed: false,
      });
      lx.showToast({ title: message, icon: "none" });
    } finally {
      if (loopId === this.mqttLoopId) {
        this.mqttSubscription = null;
        if (this.data.mqttSubscribed) {
          this.setData({
            mqttSubscribed: false,
          });
        }
      }
    }
  },
});
