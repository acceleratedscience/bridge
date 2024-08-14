/**
 * Copyright IBM Corp. 2016, 2023
 *
 * This source code is licensed under the Apache-2.0 license found in the
 * LICENSE file in the root directory of this source tree.
 */

import Dropdown, {
  type DropdownTranslationKey,
  type OnChangeData,
  type DropdownProps,
} from './Dropdown';

export type { DropdownTranslationKey, OnChangeData };
export { Dropdown, type DropdownProps };
export { default as DropdownSkeleton } from './Dropdown.Skeleton';

export default Dropdown;
