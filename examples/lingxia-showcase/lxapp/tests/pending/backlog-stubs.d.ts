export interface BacklogStub {
  id: string;
  title: string;
  mode: "planned" | "external-fixture" | "external-ui";
  covers: string[];
  reason: string;
  /** Platforms with a real replacement case in their aggregate entry. */
  implementedOn?: string[];
}

declare const stubs: BacklogStub[];
export default stubs;
