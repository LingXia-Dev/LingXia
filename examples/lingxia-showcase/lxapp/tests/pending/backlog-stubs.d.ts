export interface BacklogStub {
  id: string;
  title: string;
  mode: "planned" | "external-fixture" | "external-ui";
  covers: string[];
  reason: string;
}

declare const stubs: BacklogStub[];
export default stubs;
