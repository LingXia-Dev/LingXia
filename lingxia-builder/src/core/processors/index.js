import { VueProcessor } from "./vue.js";
import { ReactProcessor } from "./react.js";

export const PAGE_TYPES = {
  VUE: "vue",
  REACT: "react",
  HTML: "html",
};

/**
 * Processor factory
 */
export class ProcessorFactory {
  static processors = new Map([
    [PAGE_TYPES.VUE, new VueProcessor()],
    [PAGE_TYPES.REACT, new ReactProcessor()],
  ]);

  static getProcessor(pageType) {
    const processor = this.processors.get(pageType);
    if (!processor) {
      throw new Error(`No processor found for page type: ${pageType}`);
    }
    return processor;
  }

  static async process(pageType, buildDir, functions, pageFiles) {
    const processor = this.getProcessor(pageType);
    return await processor.process(buildDir, functions, pageFiles);
  }
}
