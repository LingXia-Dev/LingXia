const CLOUD_PAGE_TYPES = {
  AUTH: "auth",
  MQTT: "mqtt",
  FUNCTIONS: "functions",
} as const;

const MQTT_SHORT_TOPIC = "demo/mqtt";
const DEMO_FUNCTIONS = ["echo", "whoami", "fail"] as const;
const FUNCTION_ECHO_PAYLOAD = {
  hello: "world",
  source: "lingxia-showcase",
};

type CloudPageType =
  (typeof CLOUD_PAGE_TYPES)[keyof typeof CLOUD_PAGE_TYPES];

type TenantLike = {
  tenantId?: string;
  tenantName?: string;
  shortName?: string;
  logoUrl?: string;
};

type UserLike = {
  id: string;
  name: string;
  avatar: string;
};

type LxIdentityLike = {
  user?: {
    id?: string;
    name?: string;
    avatar?: string;
  };
  tenant?: {
    id?: string;
    name?: string;
    shortName?: string;
    logoUrl?: string;
  };
  active?: boolean;
  activate?: () => Promise<unknown>;
  logout?: () => Promise<void>;
};

type FunctionCallParams = {
  name?: string;
  payload?: any;
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
  if (raw === CLOUD_PAGE_TYPES.MQTT) {
    return CLOUD_PAGE_TYPES.MQTT;
  }
  if (raw === CLOUD_PAGE_TYPES.FUNCTIONS) {
    return CLOUD_PAGE_TYPES.FUNCTIONS;
  }
  return CLOUD_PAGE_TYPES.AUTH;
}

function getNavigationTitle(pageType: CloudPageType): string {
  switch (pageType) {
    case CLOUD_PAGE_TYPES.MQTT:
      return "Cloud MQTT Demo";
    case CLOUD_PAGE_TYPES.FUNCTIONS:
      return "Cloud Functions Demo";
    default:
      return "Cloud Auth Demo";
  }
}

function formatJson(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (value === null || value === undefined) {
    return "";
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch (_error) {
    return String(value);
  }
}

function formatMqttPayload(payload: unknown): string {
  return formatJson(payload);
}

function normalizeTenant(identityOrTenant: any): TenantLike | null {
  const tenant = identityOrTenant?.tenant || identityOrTenant;
  if (!tenant || typeof tenant !== "object") {
    return null;
  }
  return {
    tenantId: typeof tenant.id === "string"
      ? tenant.id
      : typeof tenant.tenantId === "string"
        ? tenant.tenantId
        : "",
    tenantName: typeof tenant.name === "string"
      ? tenant.name
      : typeof tenant.tenantName === "string"
        ? tenant.tenantName
        : "",
    shortName: typeof tenant.shortName === "string" ? tenant.shortName : "",
    logoUrl: typeof tenant.logoUrl === "string" ? tenant.logoUrl : "",
  };
}

function normalizeUser(identity: any): UserLike | null {
  const user = identity?.user;
  if (!user || typeof user !== "object") {
    return null;
  }
  return {
    id: typeof user.id === "string" ? user.id : "",
    name: typeof user.name === "string" ? user.name : "",
    avatar: typeof user.avatar === "string" ? user.avatar : "",
  };
}

function normalizeTenants(tenants: any[]): TenantLike[] {
  return tenants
    .map((tenant) => normalizeTenant(tenant))
    .filter((tenant): tenant is TenantLike => tenant !== null);
}

Page({
  mqttSubscription: null as any,
  mqttStatusUnsubscribe: null as any,
  mqttLoopId: 0,
  mqttStarting: false,
  pageActive: false,

  data: {
    type: CLOUD_PAGE_TYPES.AUTH,
    status: "Idle",
    tenant: null,
    user: null,
    tenants: [],
    mqttStatus: "Idle",
    mqttRuntimeState: "idle",
    mqttLastError: "",
    mqttSubscribed: false,
    mqttTopicFilter: MQTT_SHORT_TOPIC,
    mqttMessageCount: 0,
    mqttLastTopic: "",
    mqttLastPayload: "",
    mqttLastReceivedAt: "",
    functionsStatus: "Idle",
    functionsAvailable: [],
    functionsLastCall: "",
    functionsLastResult: "",
  },

  onLoad: async function (options = {}) {
    this.pageActive = true;
    const { type } = (options || {}) as { type?: string };
    const pageType = normalizePageType(type);
    if (pageType !== this.data.type) {
      this.setData({ type: pageType });
    }
    if (pageType === CLOUD_PAGE_TYPES.MQTT) {
      this._ensureMqttStatusListener();
      this._refreshMqttStatusSnapshot();
    } else {
      this._stopMqttStatusListener();
    }
    await this._applyPageType(pageType);
  },

  onShow: async function () {
    this.pageActive = true;
    const pageType = this.data.type;
    if (pageType === CLOUD_PAGE_TYPES.MQTT) {
      this._ensureMqttStatusListener();
      this._refreshMqttStatusSnapshot();
    } else {
      this._stopMqttStatusListener();
    }
    await this._applyPageType(pageType);
  },

  onHide: function () {
    this.pageActive = false;
  },

  onUnload: async function () {
    this.pageActive = false;
    this._stopMqttStatusListener();
    const subscription = this.mqttSubscription;
    this.mqttSubscription = null;
    this.mqttLoopId += 1;
    if (subscription) {
      try {
        await subscription.close();
      } catch (_error) {
        // The page is already gone; cleanup must not write back into its WebView.
      }
    }
  },

  _applyPageType: async function (pageType: CloudPageType) {
    lx.setNavigationBarTitle({
      title: getNavigationTitle(pageType),
    });

    if (pageType === CLOUD_PAGE_TYPES.MQTT) {
      this._ensureMqttStatusListener();
      this._refreshMqttStatusSnapshot();
      if (!this.data.mqttSubscribed && !this.mqttStarting) {
        void this.startMqttDemo();
      }
      return;
    }

    this._stopMqttStatusListener();

    if (pageType === CLOUD_PAGE_TYPES.FUNCTIONS) {
      await this._refreshFunctionsDemo();
      return;
    }

    await this._refreshSnapshot();
  },

  _ensureMqttStatusListener: function () {
    if (typeof this.mqttStatusUnsubscribe === "function") {
      return;
    }
    try {
      this.mqttStatusUnsubscribe = lx.cloud.mqtt.onStatusChange((nextStatus) => {
        console.log("[cloud][mqtt] onStatusChange", nextStatus);
        this._applyMqttStatus(nextStatus);
      });
    } catch (_error) {
      // Ignore status API failures so the demo remains usable.
    }
  },

  _refreshMqttStatusSnapshot: function () {
    try {
      const status = lx.cloud.mqtt.getStatus();
      console.log("[cloud][mqtt] getStatus", status);
      this._applyMqttStatus(status);
    } catch (_error) {
      // Ignore status API failures so the demo remains usable.
    }
  },

  _applyMqttStatus: function (status: any) {
    this.setData({
      mqttRuntimeState: status?.state || "idle",
      mqttLastError: typeof status?.lastError === "string" ? status.lastError : "",
    });
  },

  _stopMqttStatusListener: function () {
    const unsubscribe = this.mqttStatusUnsubscribe;
    this.mqttStatusUnsubscribe = null;
    if (typeof unsubscribe === "function") {
      try {
        unsubscribe();
      } catch (_error) {
        // Ignore cleanup errors.
      }
    }
  },

  _canUpdatePage: function () {
    if (!this.pageActive) return false;
    try {
      return getCurrentPages().includes(this);
    } catch (_error) {
      return false;
    }
  },

  _refreshSnapshot: async function () {
    try {
      const identities = await lx.auth.list();
      if (!this._canUpdatePage()) return;
      const identity = identities.find((item: LxIdentityLike) => item.active) || null;
      this.setData({
        status: identity
          ? "Ready"
          : "Authentication required. Call lx.auth.login() first.",
        tenant: normalizeTenant(identity),
        user: normalizeUser(identity),
        tenants: normalizeTenants(identities),
      });
    } catch (error) {
      if (!this._canUpdatePage()) return;
      const message = getErrorMessage(error, "Load cloud state failed");
      this.setData({ status: message });
    }
  },

  _refreshFunctionsDemo: async function () {
    let identity: LxIdentityLike | null | undefined = null;
    try {
      const identities = await lx.auth.list();
      identity = identities.find((item: LxIdentityLike) => item.active);
    } catch (_error) {
      identity = null;
    }
    if (!this._canUpdatePage()) return;
    this.setData({
      functionsAvailable: [...DEMO_FUNCTIONS],
      functionsStatus: identity
        ? "Ready to invoke current lxapp cloud functions."
        : "Authentication required. Call lx.auth.login() first.",
    });
    await this._refreshSnapshot();
  },

  loginInteractive: async function () {
    this.setData({ status: "Starting login..." });
    try {
      await lx.auth.login();
      this.setData({ status: "Login succeeded" });
      await this._refreshSnapshot();
      await this._refreshFunctionsDemo();
    } catch (error) {
      this.setData({ status: getErrorMessage(error, "Interactive login failed") });
    }
  },

  addTenant: async function () {
    this.setData({ status: "Adding identity..." });
    try {
      await lx.auth.add();
      this.setData({ status: "Identity added" });
      await this._refreshSnapshot();
      await this._refreshFunctionsDemo();
    } catch (error) {
      this.setData({ status: getErrorMessage(error, "Add identity failed") });
    }
  },

  logoutCurrentTenant: async function () {
    this.setData({ status: "Logging out..." });
    try {
      await this.stopMqttDemo();
      const identities = await lx.auth.list();
      const activeIdentity = identities.find((identity: LxIdentityLike) => identity.active);
      if (!activeIdentity || typeof activeIdentity.logout !== "function") {
        throw new Error("No active identity to logout");
      }
      await activeIdentity.logout();
      this.setData({ status: "Logged out" });
      this._refreshMqttStatusSnapshot();
      await this._refreshSnapshot();
      await this._refreshFunctionsDemo();
    } catch (error) {
      this.setData({ status: getErrorMessage(error, "Logout failed") });
    }
  },

  activateTenant: async function (params = {}) {
    const { tenantId } = params as { tenantId?: string };
    if (!tenantId) return;
    this.setData({ status: `Activating ${tenantId}...` });
    try {
      const identities = await lx.auth.list();
      const identity = identities.find((item: LxIdentityLike) => item.tenant?.id === tenantId);
      if (!identity || typeof identity.activate !== "function") {
        throw new Error(`Identity ${tenantId} not found`);
      }
      await identity.activate();
      this.setData({ status: `Activated ${tenantId}` });
      await this._refreshSnapshot();
      await this._refreshFunctionsDemo();
    } catch (error) {
      this.setData({ status: getErrorMessage(error, "Activate tenant failed") });
    }
  },

  _callCloudFunction: async function (params = {}) {
    const { name, payload } = params as FunctionCallParams;
    const functionName = typeof name === "string" ? name.trim() : "";
    if (!functionName) {
      return;
    }

    this.setData({
      functionsStatus: `Calling ${functionName}...`,
      functionsLastCall: functionName,
    });
    try {
      const result = await lx.cloud.invoke(functionName, payload);
      this.setData({
        functionsStatus: `${functionName} succeeded`,
        functionsLastCall: functionName,
        functionsLastResult: formatJson(result),
      });
    } catch (error) {
      this.setData({
        functionsStatus: getErrorMessage(error, `${functionName} failed`),
        functionsLastCall: functionName,
        functionsLastResult: "",
      });
    }
  },

  callNamedFunction: async function (params = {}) {
    const { name } = params as { name?: string };
    const functionName = typeof name === "string" ? name.trim() : "";
    if (!functionName) {
      return;
    }

    const payload =
      functionName === "echo" ? FUNCTION_ECHO_PAYLOAD : null;
    await this._callCloudFunction({ name: functionName, payload });
  },

  startMqttDemo: async function () {
    if (this.mqttSubscription || this.mqttStarting) {
      return;
    }
    this.mqttStarting = true;
    this._ensureMqttStatusListener();
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
      this._refreshMqttStatusSnapshot();
      void this._consumeMqtt({ subscription, loopId });
    } catch (error) {
      const message = getErrorMessage(error, "MQTT subscribe failed");
      this.setData({
        mqttStatus: message,
        mqttSubscribed: false,
      });
    } finally {
      this.mqttStarting = false;
    }
  },

  stopMqttDemo: async function () {
    const subscription = this.mqttSubscription;
    this.mqttSubscription = null;
    this.mqttLoopId += 1;
    if (!subscription) {
      this.setData({
        mqttStatus: "Demo subscription is inactive",
        mqttSubscribed: false,
      });
      this._refreshMqttStatusSnapshot();
      return;
    }
    try {
      await subscription.close();
      this.setData({
        mqttStatus: "Demo subscription stopped",
        mqttSubscribed: false,
      });
      this._refreshMqttStatusSnapshot();
    } catch (_error) {
      // Ignore duplicate cleanup while leaving the page.
    }
  },

  _consumeMqtt: async function ({
    subscription,
    loopId,
  }: {
    subscription: any;
    loopId: number;
  }) {
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
      this._refreshMqttStatusSnapshot();
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
