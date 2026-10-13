import { NodeTypes, type NodeTransform } from '@vue/compiler-core';
import type { Plugin } from 'vite';

export const TEST_ID_ATTRIBUTE = 'data-testid';

/**
 * Removes data-testid attributes from Vue templates during production builds.
 * Tests and Playwright keep the attributes while running in development.
 */
export const stripTestIdsNodeTransform: NodeTransform = (node) => {
  if (node.type !== NodeTypes.ELEMENT) {
    return;
  }

  node.props = node.props.filter((prop) => {
    if (prop.type === NodeTypes.ATTRIBUTE) {
      return prop.name !== TEST_ID_ATTRIBUTE;
    }

    return !(
      prop.name === 'bind' &&
      prop.arg?.type === NodeTypes.SIMPLE_EXPRESSION &&
      prop.arg.isStatic &&
      prop.arg.content === TEST_ID_ATTRIBUTE
    );
  });
};

/**
 * Removes data-testid attributes from the static HTML served by Vite.
 */
export function stripTestIds(): Plugin {
  return {
    name: 'kickstart:strip-test-ids',
    apply: 'build',
    enforce: 'post',
    transformIndexHtml(html) {
      return html.replaceAll(new RegExp(`\\s${TEST_ID_ATTRIBUTE}="[^"]*"`, 'g'), '');
    },
  };
}
