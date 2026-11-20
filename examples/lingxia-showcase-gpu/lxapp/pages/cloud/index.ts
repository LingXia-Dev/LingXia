import { authApi, cloudApi } from "../../shared/lib/provider";
import { definePage } from '@lingxia/types/page';
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

export default definePage({
  data: {
    type: CLOUD_PAGE_TYPES.AUTH as CloudPageType,
    status: "Idle",
    tenant: null as TenantLike | null,
    user: null as UserLike | null,
    tenants: [] as TenantLike[],
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
    functionsAvailable: [] as (typeof DEMO_FUNCTIONS)[number][],
    functionsLastCall: "",
    functionsLastResult: "",
  },
  setup(page) {
    let mqttSubscription = null as any;
    let mqttStatusUnsubscribe = null as any;
    let mqttLoopId = 0;
    let mqttStarting = false;
    let functionRequestId = 0;
    let pageActive = false;
    const helpers = {
      async applyPageType(pageType: CloudPageType) {
        void lx.navigationBar.update({
          title: getNavigationTitle(pageType),
        });

        if (pageType === CLOUD_PAGE_TYPES.MQTT) {
          helpers.ensureMqttStatusListener();
          helpers.refreshMqttStatusSnapshot();
          if (!page.data.mqttSubscribed && !mqttStarting) {
            void actions.startMqttDemo();
          }
          return;
        }

        helpers.stopMqttStatusListener();

        if (pageType === CLOUD_PAGE_TYPES.FUNCTIONS) {
          await helpers.refreshFunctionsDemo();
          return;
        }

        await helpers.refreshSnapshot();
      },
      ensureMqttStatusListener() {
        if (typeof mqttStatusUnsubscribe === "function") {
          return;
        }
        try {
          mqttStatusUnsubscribe = cloudApi().mqtt.onStatusChange((nextStatus) => {
            console.log("[cloud][mqtt] onStatusChange", nextStatus);
            helpers.applyMqttStatus(nextStatus);
          });
        } catch (_error) {
          // Ignore status API failures so the demo remains usable.
        }
      },
      refreshMqttStatusSnapshot() {
        try {
          const status = cloudApi().mqtt.getStatus();
          console.log("[cloud][mqtt] getStatus", status);
          helpers.applyMqttStatus(status);
        } catch (_error) {
          // Ignore status API failures so the demo remains usable.
        }
      },
      applyMqttStatus(status: any) {
        page.update(d => {
          d.mqttRuntimeState = status?.state || "idle";
          d.mqttLastError = typeof status?.lastError === "string" ? status.lastError : "";
        });
      },
      stopMqttStatusListener() {
        const unsubscribe = mqttStatusUnsubscribe;
        mqttStatusUnsubscribe = null;
        if (typeof unsubscribe === "function") {
          try {
            unsubscribe();
          } catch (_error) {
            // Ignore cleanup errors.
          }
        }
      },
      canUpdatePage() {
        if (!pageActive) return false;
        try {
          return !page.signal.aborted;
        } catch (_error) {
          return false;
        }
      },
      async refreshSnapshot() {
        try {
          const identities = await authApi().list();
          if (!helpers.canUpdatePage()) return;
          const identity = identities.find((item: LxIdentityLike) => item.active) || null;
          page.update(d => {
            d.status = identity
              ? "Ready"
              : "Authentication required. Call lx.auth.login() first.";
            d.tenant = normalizeTenant(identity);
            d.user = normalizeUser(identity);
            d.tenants = normalizeTenants(identities);
          });
        } catch (error) {
          if (!helpers.canUpdatePage()) return;
          const message = getErrorMessage(error, "Load cloud state failed");
          page.update(d => { d.status = message; });
        }
      },
      async refreshFunctionsDemo() {
        let identity: LxIdentityLike | null | undefined = null;
        try {
          const identities = await authApi().list();
          identity = identities.find((item: LxIdentityLike) => item.active);
        } catch (_error) {
          identity = null;
        }
        if (!helpers.canUpdatePage()) return;
        page.update(d => {
          d.functionsAvailable = [...DEMO_FUNCTIONS];
          d.functionsStatus = identity
            ? "Ready to invoke current lxapp cloud functions."
            : "Authentication required. Call lx.auth.login() first.";
        });
        await helpers.refreshSnapshot();
      },
      async callCloudFunction(params = {}) {
        const { name, payload } = params as FunctionCallParams;
        const functionName = typeof name === "string" ? name.trim() : "";
        if (!functionName) {
          return;
        }

        const requestId = ++functionRequestId;
        page.update(d => {
          d.functionsStatus = `Calling ${functionName}...`;
          d.functionsLastCall = functionName;
          d.functionsLastResult = "";
        });
        try {
          const result = await cloudApi().invoke(functionName, payload);
          if (requestId !== functionRequestId) return;
          page.update(d => {
            d.functionsStatus = `${functionName} succeeded`;
            d.functionsLastCall = functionName;
            d.functionsLastResult = formatJson(result);
          });
        } catch (error) {
          if (requestId !== functionRequestId) return;
          page.update(d => {
            d.functionsStatus = getErrorMessage(error, `${functionName} failed`);
            d.functionsLastCall = functionName;
            d.functionsLastResult = "";
          });
        }
      },
      async consumeMqtt({
        subscription,
        loopId,
      }: {
        subscription: any;
        loopId: number;
      }) {
        try {
          for await (const message of subscription) {
            if (loopId !== mqttLoopId) {
              break;
            }
            console.log("[cloud][mqtt] received", {
              topic: message.topic,
              payload: message.payload,
              qos: message.qos,
              receivedAt: message.receivedAt,
            });
            const nextCount = page.data.mqttMessageCount + 1;
            page.update(d => {
              d.mqttStatus = `Received message #${nextCount}`;
              d.mqttMessageCount = nextCount;
              d.mqttLastTopic = message.topic;
              d.mqttLastPayload = formatMqttPayload(message.payload);
              d.mqttLastReceivedAt = new Date(message.receivedAt).toLocaleString();
            });
          }
        } catch (error) {
          if (loopId !== mqttLoopId) {
            return;
          }
          const message = getErrorMessage(error, "MQTT stream failed");
          page.update(d => {
            d.mqttStatus = message;
            d.mqttSubscribed = false;
          });
          helpers.refreshMqttStatusSnapshot();
        } finally {
          if (loopId === mqttLoopId) {
            mqttSubscription = null;
            if (page.data.mqttSubscribed) {
              page.update(d => { d.mqttSubscribed = false; });
            }
          }
        }
      },
    };
    const actions = {
      async goBack() { await lx.nav.back(); },
      async loginInteractive() {
        page.update(d => { d.status = "Starting login..."; });
        try {
          await authApi().login();
          page.update(d => { d.status = "Login succeeded"; });
          await helpers.refreshSnapshot();
          await helpers.refreshFunctionsDemo();
        } catch (error) {
          page.update(d => { d.status = getErrorMessage(error, "Interactive login failed"); });
        }
      },
      async addTenant() {
        page.update(d => { d.status = "Adding identity..."; });
        try {
          await authApi().add();
          page.update(d => { d.status = "Identity added"; });
          await helpers.refreshSnapshot();
          await helpers.refreshFunctionsDemo();
        } catch (error) {
          page.update(d => { d.status = getErrorMessage(error, "Add identity failed"); });
        }
      },
      async logoutCurrentTenant() {
        page.update(d => { d.status = "Logging out..."; });
        try {
          await actions.stopMqttDemo();
          const identities = await authApi().list();
          const activeIdentity = identities.find((identity: LxIdentityLike) => identity.active);
          if (!activeIdentity || typeof activeIdentity.logout !== "function") {
            throw new Error("No active identity to logout");
          }
          await activeIdentity.logout();
          page.update(d => { d.status = "Logged out"; });
          helpers.refreshMqttStatusSnapshot();
          await helpers.refreshSnapshot();
          await helpers.refreshFunctionsDemo();
        } catch (error) {
          page.update(d => { d.status = getErrorMessage(error, "Logout failed"); });
        }
      },
      async activateTenant(params = {}) {
        const { tenantId } = params as { tenantId?: string };
        if (!tenantId) return;
        page.update(d => { d.status = `Activating ${tenantId}...`; });
        try {
          const identities = await authApi().list();
          const identity = identities.find((item: LxIdentityLike) => item.tenant?.id === tenantId);
          if (!identity || typeof identity.activate !== "function") {
            throw new Error(`Identity ${tenantId} not found`);
          }
          await identity.activate();
          page.update(d => { d.status = `Activated ${tenantId}`; });
          await helpers.refreshSnapshot();
          await helpers.refreshFunctionsDemo();
        } catch (error) {
          page.update(d => { d.status = getErrorMessage(error, "Activate tenant failed"); });
        }
      },
      async callNamedFunction(params = {}) {
        const { name } = params as { name?: string };
        const functionName = typeof name === "string" ? name.trim() : "";
        if (!functionName) {
          return;
        }

        const payload =
          functionName === "echo" ? FUNCTION_ECHO_PAYLOAD : null;
        await helpers.callCloudFunction({ name: functionName, payload });
      },
      async startMqttDemo() {
        if (mqttSubscription || mqttStarting) {
          return;
        }
        mqttStarting = true;
        const loopId = ++mqttLoopId;
        helpers.ensureMqttStatusListener();
        page.update(d => { d.mqttStatus = `Subscribing to ${MQTT_SHORT_TOPIC}...`; });
        try {
          const subscription = await cloudApi().mqtt.subscribe(MQTT_SHORT_TOPIC, {
            parse: "auto",
          });
          // Stop or unload can invalidate a request before its handle arrives.
          if (loopId !== mqttLoopId) {
            await subscription.close();
            return;
          }
          mqttSubscription = subscription;
          page.update(d => {
            d.mqttStatus = `Subscribed to ${MQTT_SHORT_TOPIC}`;
            d.mqttSubscribed = true;
          });
          helpers.refreshMqttStatusSnapshot();
          void helpers.consumeMqtt({ subscription, loopId });
        } catch (error) {
          if (loopId !== mqttLoopId) return;
          const message = getErrorMessage(error, "MQTT subscribe failed");
          page.update(d => {
            d.mqttStatus = message;
            d.mqttSubscribed = false;
          });
        } finally {
          if (loopId === mqttLoopId) mqttStarting = false;
        }
      },
      async stopMqttDemo() {
        const subscription = mqttSubscription;
        mqttSubscription = null;
        const loopId = ++mqttLoopId;
        mqttStarting = false;
        if (!subscription) {
          page.update(d => {
            d.mqttStatus = "Demo subscription is inactive";
            d.mqttSubscribed = false;
          });
          helpers.refreshMqttStatusSnapshot();
          return;
        }
        try {
          await subscription.close();
          if (loopId !== mqttLoopId) return;
          page.update(d => {
            d.mqttStatus = "Demo subscription stopped";
            d.mqttSubscribed = false;
          });
          helpers.refreshMqttStatusSnapshot();
        } catch (_error) {
          // Ignore duplicate cleanup while leaving the page.
        }
      },
    };
    return {
      async onLoad(options = {}) {
        pageActive = true;
        const { type } = (options || {}) as { type?: string };
        const pageType = normalizePageType(type);
        if (pageType !== page.data.type) {
          page.update(d => { d.type = pageType; });
        }
        if (pageType === CLOUD_PAGE_TYPES.MQTT) {
          helpers.ensureMqttStatusListener();
          helpers.refreshMqttStatusSnapshot();
        } else {
          helpers.stopMqttStatusListener();
        }
        await helpers.applyPageType(pageType);
      },
      async onShow() {
        pageActive = true;
        const pageType = page.data.type;
        if (pageType === CLOUD_PAGE_TYPES.MQTT) {
          helpers.ensureMqttStatusListener();
          helpers.refreshMqttStatusSnapshot();
        } else {
          helpers.stopMqttStatusListener();
        }
        await helpers.applyPageType(pageType);
      },
      onHide() {
        pageActive = false;
      },
      async onUnload() {
        pageActive = false;
        functionRequestId += 1;
        helpers.stopMqttStatusListener();
        const subscription = mqttSubscription;
        mqttSubscription = null;
        mqttLoopId += 1;
        mqttStarting = false;
        if (subscription) {
          try {
            await subscription.close();
          } catch (_error) {
            // The page is already gone; cleanup must not write back into its View.
          }
        }
      },
      actions,
    };
  },
});
