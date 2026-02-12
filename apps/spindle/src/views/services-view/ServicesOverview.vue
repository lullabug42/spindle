<!--
  ServicesOverview: Lists all service groups as cards (grid/list). Starts polling on mount,
  stops on unmount. Clicking a service navigates to that group's detail with service pre-selected.
  Provides functionality to add new services via a modal form with comprehensive validation.
-->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from "vue";
import { useRouter } from "vue-router";
import type { FormRules, FormInst } from "naive-ui";
import {
  NButton,
  NSpace,
  NFlex,
  NSwitch,
  NCard,
  NModal,
  NForm,
  NFormItem,
  NInput,
  NIcon,
  useMessage,
} from "naive-ui";
import {
  GridViewOutlined,
  ViewListOutlined,
  AddCircleFilled,
  DeleteOutlineOutlined,
} from "@vicons/material";
import ServiceGroupCard from "@/components/services-view/ServiceGroupCard.vue";
import { useServiceStore } from "@/stores/serviceStore";
import type { ServiceItem as ServiceItemType } from "@/types/service.types";


/** Regex for validating non-whitespace input. */
const NON_WHITESPACE_REGEX = /^[^\s]*$/;

/** Regex for validating service name (letters, numbers, hyphens, underscores, dots). */
const SERVICE_NAME_REGEX = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;

/** Regex for validating SemVer format (basic: major.minor.patch). */
const SEMVER_REGEX = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)*))?(?:\+([a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)*))?$/;

/** Regex for validating file/directory paths (no leading/trailing spaces, basic path chars). */
const PATH_REGEX = /^[^\s].*[^\s]$|^[^\s]$/;

/** Maximum length constants for form fields. */
const MAX_LENGTH = {
  name: 64,
  version: 32,
  program: 512,
  description: 256,
  workspace: 512,
  arg: 256,
  depName: 64,
  depVersion: 32,
} as const;

/** Vue router instance for navigation. */
const router = useRouter();

/** Service store for managing groups and polling. */
const store = useServiceStore();

/** Message API for displaying success/error notifications. */
const message = useMessage();

/** Reference to the form instance for validation. */
const formRef = ref<FormInst | null>(null);

/** Controls visibility of the add service modal. */
const showAddModal = ref(false);

/** Loading state for form submission. */
const isSubmitting = ref(false);

/** Represents a single dependency item in the form. */
interface DependencyItem {
  /** Service name of the dependency. */
  name: string;
  /** Version of the dependency. */
  version: string;
}

/** Form data for creating a new service. */
const formData = ref({
  /** Service name. */
  name: "",
  /** Service version. */
  version: "",
  /** Path to the executable. */
  program: "",
  /** Optional description. */
  description: "",
  /** Optional working directory. */
  workspace: "",
  /** List of command line arguments. */
  args: [] as string[],
  /** List of dependencies. */
  dependencies: [] as DependencyItem[],
});

/** Computed set of existing service identifiers for dependency validation. */
const existingServices = computed(() => {
  const services = new Set<string>();
  // Include services from groups
  for (const group of store.groups) {
    for (const svc of group.services) {
      services.add(`${svc.name}:${svc.version}`);
    }
  }
  // Also include pending services (for dependency checking)
  for (const svc of store.pendingServices) {
    services.add(`${svc.name}:${svc.version}`);
  }
  return services;
});

/** Form validation rules following Naive UI FormRules format. */
const formRules: FormRules = {
  name: [
    { required: true, message: "Service name is required", trigger: ["input", "blur"] },
    {
      validator: (_rule, value: string) => {
        if (!value) return true;
        if (!SERVICE_NAME_REGEX.test(value)) {
          return new Error("Name must start with letter/number, only allow letters, numbers, hyphens, underscores, dots");
        }
        if (value.length > MAX_LENGTH.name) {
          return new Error(`Name must not exceed ${MAX_LENGTH.name} characters`);
        }
        return true;
      },
      trigger: ["input", "blur"],
    },
  ],
  version: [
    { required: true, message: "Version is required", trigger: ["input", "blur"] },
    {
      validator: (_rule, value: string) => {
        if (!value) return true;
        if (!SEMVER_REGEX.test(value)) {
          return new Error("Version must follow SemVer format (e.g., 1.0.0, 1.0.0-alpha.1)");
        }
        if (value.length > MAX_LENGTH.version) {
          return new Error(`Version must not exceed ${MAX_LENGTH.version} characters`);
        }
        return true;
      },
      trigger: ["input", "blur"],
    },
  ],
  program: [
    { required: true, message: "Program path is required", trigger: ["input", "blur"] },
    {
      validator: (_rule, value: string) => {
        if (!value) return true;
        if (!PATH_REGEX.test(value)) {
          return new Error("Program path cannot start or end with spaces");
        }
        if (value.length > MAX_LENGTH.program) {
          return new Error(`Program path must not exceed ${MAX_LENGTH.program} characters`);
        }
        return true;
      },
      trigger: ["input", "blur"],
    },
  ],
  description: [
    {
      validator: (_rule, value: string) => {
        if (!value) return true;
        if (value.length > MAX_LENGTH.description) {
          return new Error(`Description must not exceed ${MAX_LENGTH.description} characters`);
        }
        return true;
      },
      trigger: ["input", "blur"],
    },
  ],
  workspace: [
    {
      validator: (_rule, value: string) => {
        if (!value) return true;
        if (!PATH_REGEX.test(value)) {
          return new Error("Workspace path cannot start or end with spaces");
        }
        if (value.length > MAX_LENGTH.workspace) {
          return new Error(`Workspace path must not exceed ${MAX_LENGTH.workspace} characters`);
        }
        return true;
      },
      trigger: ["input", "blur"],
    },
  ],
};

/** Starts polling service status on component mount. */
onMounted(() => {
  store.startPolling();
});

/** Stops polling service status on component unmount. */
onUnmounted(() => {
  store.stopPolling();
});

/**
 * Navigates to the group detail page and opens the service in the detail modal via query.
 * @param service - The clicked service (used for group_id and name@version query).
 */
function onServiceClick(service: ServiceItemType): void {
  // Services in groups should always have a group_id
  if (service.group_id === null) {
    message.warning("Service is not yet assigned to a group");
    return;
  }
  router.push({
    name: "ServiceGroupDetail",
    params: { groupId: String(service.group_id) },
    query: { service: `${service.name}@${service.version}` },
  });
}

/**
 * Toggles between mock and real data mode.
 * @param value - True to use mock data, false for real data.
 */
async function onMockSwitch(value: boolean): Promise<void> {
  await store.setUseMock(value);
}

/** Opens the add service modal. */
function openAddModal(): void {
  showAddModal.value = true;
}

/** Closes the add service modal and resets the form. */
function closeAddModal(): void {
  showAddModal.value = false;
  resetForm();
}

/** Resets the form data to initial empty state. */
function resetForm(): void {
  formData.value = {
    name: "",
    version: "",
    program: "",
    description: "",
    workspace: "",
    args: [],
    dependencies: [],
  };
}

/** Adds a new empty argument field to the form. */
function addArg(): void {
  formData.value.args.push("");
}

/**
 * Removes an argument field at the specified index.
 * @param index - Index of the argument to remove.
 */
function removeArg(index: number): void {
  formData.value.args.splice(index, 1);
}

/**
 * Validates a single argument value.
 * @param value - The argument value to validate.
 * @returns Error message if invalid, null if valid.
 */
function validateArg(value: string): string | null {
  if (value.length > MAX_LENGTH.arg) {
    return `Max ${MAX_LENGTH.arg} chars`;
  }
  if (!NON_WHITESPACE_REGEX.test(value)) {
    return "No whitespace";
  }
  return null;
}

/** Adds a new empty dependency item to the form. */
function addDependency(): void {
  formData.value.dependencies.push({ name: "", version: "" });
}

/**
 * Removes a dependency item at the specified index.
 * @param index - Index of the dependency to remove.
 */
function removeDependency(index: number): void {
  formData.value.dependencies.splice(index, 1);
}

/**
 * Validates a dependency name field.
 * @param value - The name value to validate.
 * @returns Error message if invalid, null if valid.
 */
function validateDepName(value: string): string | null {
  if (!value) return null;
  if (value.length > MAX_LENGTH.depName) {
    return `Max ${MAX_LENGTH.depName} chars`;
  }
  if (!NON_WHITESPACE_REGEX.test(value)) {
    return "No whitespace";
  }
  if (!SERVICE_NAME_REGEX.test(value)) {
    return "Invalid format";
  }
  return null;
}

/**
 * Validates a dependency version field.
 * @param value - The version value to validate.
 * @returns Error message if invalid, null if valid.
 */
function validateDepVersion(value: string): string | null {
  if (!value) return null;
  if (value.length > MAX_LENGTH.depVersion) {
    return `Max ${MAX_LENGTH.depVersion} chars`;
  }
  if (!NON_WHITESPACE_REGEX.test(value)) {
    return "No whitespace";
  }
  if (!SEMVER_REGEX.test(value)) {
    return "Invalid SemVer";
  }
  return null;
}

/**
 * Checks if a dependency refers to an existing service.
 * @param dep - The dependency item to check.
 * @returns True if the dependency exists in current services.
 */
function isDependencyExisting(dep: DependencyItem): boolean {
  if (!dep.name || !dep.version) return true;
  return existingServices.value.has(`${dep.name}:${dep.version}`);
}

/**
 * Checks if any argument has validation errors.
 * @returns True if all arguments are valid.
 */
function validateAllArgs(): boolean {
  return formData.value.args.every((arg) => !validateArg(arg));
}

/**
 * Checks if any dependency field has validation errors.
 * @returns True if all dependencies are valid.
 */
function validateAllDependencies(): boolean {
  return formData.value.dependencies.every(
    (dep) => !validateDepName(dep.name) && !validateDepVersion(dep.version)
  );
}

/**
 * Validates form input and submits the new service.
 * On success, closes the modal and refreshes the service list.
 */
async function handleSubmit(): Promise<void> {
  if (!formRef.value) return;

  // Validate basic form fields
  try {
    await formRef.value.validate();
  } catch {
    message.error("Please fix validation errors");
    return;
  }

  // Validate dynamic args
  if (!validateAllArgs()) {
    message.error("Please fix argument validation errors");
    return;
  }

  // Validate dynamic dependencies
  if (!validateAllDependencies()) {
    message.error("Please fix dependency validation errors");
    return;
  }

  // Validate dependencies exist
  const nonExistingDeps = formData.value.dependencies.filter(
    (dep) => dep.name && dep.version && !isDependencyExisting(dep)
  );
  if (nonExistingDeps.length > 0) {
    const depList = nonExistingDeps.map((d) => `${d.name}:${d.version}`).join(", ");
    message.error(`Dependencies not found: ${depList}`);
    return;
  }

  isSubmitting.value = true;
  try {
    const args = formData.value.args.filter(Boolean);
    const dependencies: [string, string][] = formData.value.dependencies
      .filter((dep) => dep.name && dep.version)
      .map((dep) => [dep.name, dep.version]);

    await store.addServiceToStore({
      name: formData.value.name,
      version: formData.value.version,
      program: formData.value.program,
      description: formData.value.description || null,
      workspace: formData.value.workspace || null,
      args,
      dependencies,
    });

    // In real mode, reload service manager to sync service_group_membership
    // This ensures the newly added service is properly assigned to groups
    if (!store.useMock) {
      await store.reloadServiceManager();
    }

    message.success("Service added successfully");
    closeAddModal();
    // Note: fetchGroups is now called inside addServiceToStore for real mode
    // In mock mode, the service is added directly to groups.value
  } catch (error) {
    message.error(`Failed to add service: ${error}`);
  } finally {
    isSubmitting.value = false;
  }
}

/**
 * Input validator that only allows non-whitespace characters.
 * @param value - The input value to validate.
 * @returns True if the value contains no whitespace.
 */
function allowNonWhitespace(value: string): boolean {
  return NON_WHITESPACE_REGEX.test(value);
}
</script>

<template>
  <div>
    <!-- Main content area -->
    <n-flex vertical :size="16">
      <!-- Header with title and view controls -->
      <n-flex align="center" justify="space-between">
        <h2 class="page-title">Services</h2>
        <n-space>
          <n-button quaternary :type="store.overviewViewMode === 'grid' ? 'primary' : undefined"
            @click="store.overviewViewMode = 'grid'">
            <template #icon>
              <n-icon :component="GridViewOutlined" />
            </template>
            Grid
          </n-button>
          <n-button quaternary :type="store.overviewViewMode === 'list' ? 'primary' : undefined"
            @click="store.overviewViewMode = 'list'">
            <template #icon>
              <n-icon :component="ViewListOutlined" />
            </template>
            List
          </n-button>
          <n-button type="primary" @click="openAddModal">
            <template #icon>
              <n-icon :component="AddCircleFilled" />
            </template>
            Add Service
          </n-button>
        </n-space>
      </n-flex>

      <!-- Loading state -->
      <n-flex v-if="store.loading && store.groups.length === 0" justify="center">
        Loading...
      </n-flex>

      <!-- Service groups list -->
      <n-flex v-else vertical :size="16">
        <ServiceGroupCard v-for="group in store.groups" :key="group.group_id" :group="group"
          :view-mode="store.overviewViewMode" @service-click="onServiceClick" />
      </n-flex>

      <!-- Mock data toggle card -->
      <n-card class="mock-toggle-card" size="small" embedded>
        <n-flex align="center" justify="space-between" class="mock-toggle-inner">
          <div class="mock-toggle-label">
            <span class="mock-toggle-title">Use Mock Data</span>
            <span class="mock-toggle-desc">
              {{
                store.useMock
                  ? "Show local mock service groups; no backend required."
                  : "Load real data from the service manager."
              }}
            </span>
          </div>
          <n-switch :value="store.useMock" @update:value="onMockSwitch" />
        </n-flex>
      </n-card>
    </n-flex>

    <!-- Add Service Modal -->
    <n-modal v-model:show="showAddModal" preset="card" title="Add New Service" style="width: 50vw; min-width: 24rem;"
      :mask-closable="false" @close="closeAddModal">
      <n-form ref="formRef" :model="formData" :rules="formRules" label-placement="left" label-width="8rem">
        <!-- Basic Info Section -->
        <n-form-item label="Name" path="name" required>
          <n-input v-model:value="formData.name" placeholder="Service name" :maxlength="MAX_LENGTH.name" show-count />
        </n-form-item>

        <n-form-item label="Version" path="version" required>
          <n-input v-model:value="formData.version" placeholder="e.g. 1.0.0" :maxlength="MAX_LENGTH.version"
            show-count />
        </n-form-item>

        <n-form-item label="Program" path="program" required>
          <n-input v-model:value="formData.program" placeholder="Path to executable" :maxlength="MAX_LENGTH.program" />
        </n-form-item>

        <n-form-item label="Description" path="description">
          <n-input v-model:value="formData.description" placeholder="Optional description" type="textarea" :rows="2"
            :maxlength="MAX_LENGTH.description" show-count />
        </n-form-item>

        <n-form-item label="Workspace" path="workspace">
          <n-input v-model:value="formData.workspace" placeholder="Optional working directory"
            :maxlength="MAX_LENGTH.workspace" />
        </n-form-item>

        <!-- Arguments Section -->
        <n-form-item label="Args">
          <div class="dynamic-list">
            <div v-for="(arg, index) in formData.args" :key="`arg-${index}`" class="dynamic-list-item">
              <n-input v-model:value="formData.args[index]" placeholder="Argument value" size="small"
                :maxlength="MAX_LENGTH.arg" :allow-input="allowNonWhitespace"
                :status="arg && (arg.length > MAX_LENGTH.arg || !NON_WHITESPACE_REGEX.test(arg)) ? 'error' : undefined" />
              <span v-if="arg && (arg.length > MAX_LENGTH.arg || !NON_WHITESPACE_REGEX.test(arg))" class="field-error">
                {{ arg.length > MAX_LENGTH.arg ? `Max ${MAX_LENGTH.arg} chars` : 'No whitespace' }}
              </span>
              <n-button quaternary circle size="small" class="delete-btn" @click="removeArg(index)">
                <template #icon>
                  <n-icon :component="DeleteOutlineOutlined" />
                </template>
              </n-button>
            </div>
            <n-button dashed size="small" class="add-btn" @click="addArg">
              <template #icon>
                <n-icon :component="AddCircleFilled" />
              </template>
              Add Argument
            </n-button>
          </div>
        </n-form-item>

        <!-- Dependencies Section -->
        <n-form-item label="Dependencies">
          <div class="dynamic-list">
            <div v-for="(dep, index) in formData.dependencies" :key="`dep-${index}`" class="dynamic-list-item"
              :class="{ 'dep-warning': !isDependencyExisting(dep) && dep.name && dep.version }">
              <n-input v-model:value="dep.name" placeholder="Name" size="small" :maxlength="MAX_LENGTH.depName"
                :allow-input="allowNonWhitespace" :status="dep.name && (
                  dep.name.length > MAX_LENGTH.depName ||
                  !NON_WHITESPACE_REGEX.test(dep.name) ||
                  !SERVICE_NAME_REGEX.test(dep.name)
                ) ? 'error' : undefined" />
              <n-input v-model:value="dep.version" placeholder="Version" size="small" class="version-input"
                :maxlength="MAX_LENGTH.depVersion" :allow-input="allowNonWhitespace" :status="dep.version && (
                  dep.version.length > MAX_LENGTH.depVersion ||
                  !NON_WHITESPACE_REGEX.test(dep.version) ||
                  !SEMVER_REGEX.test(dep.version)
                ) ? 'error' : undefined" />
              <n-button quaternary circle size="small" class="delete-btn" @click="removeDependency(index)">
                <template #icon>
                  <n-icon :component="DeleteOutlineOutlined" />
                </template>
              </n-button>
              <span v-if="!isDependencyExisting(dep) && dep.name && dep.version" class="dep-not-found">
                Not found
              </span>
            </div>
            <n-button dashed size="small" class="add-btn" @click="addDependency">
              <template #icon>
                <n-icon :component="AddCircleFilled" />
              </template>
              Add Dependency
            </n-button>
          </div>
        </n-form-item>
      </n-form>

      <template #footer>
        <n-space justify="end">
          <n-button @click="closeAddModal">Cancel</n-button>
          <n-button type="primary" :loading="isSubmitting" @click="handleSubmit">
            Add Service
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
/** Page title styles. */
.page-title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
}

/** Mock data toggle card styles. */
.mock-toggle-card {
  margin-top: 0.5rem;
  border-radius: 0.625rem;
  border: 1px solid var(--n-border-color);
  background: var(--n-color-modal);
}

.mock-toggle-inner {
  width: 100%;
}

.mock-toggle-label {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

.mock-toggle-title {
  font-weight: 600;
  font-size: 0.875rem;
  color: var(--n-text-color);
}

.mock-toggle-desc {
  font-size: 0.75rem;
  color: var(--n-text-color-3);
}

/** Dynamic list container for args and dependencies. */
.dynamic-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

/** Individual item in the dynamic list. */
.dynamic-list-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.dynamic-list-item :deep(.n-input) {
  flex: 1;
}

/** Version input field - fixed width. */
.version-input {
  flex: 0 0 5rem !important;
}

/** Delete button - subtle appearance. */
.delete-btn {
  opacity: 0.6;
  transition: opacity 0.2s;
  flex-shrink: 0;
}

.delete-btn:hover {
  opacity: 1;
}

/** Add button - full width dashed style. */
.add-btn {
  width: 100%;
  justify-content: center;
}

/** Field error message for dynamic fields. */
.field-error {
  font-size: 0.75rem;
  color: var(--n-color-error);
  flex-shrink: 0;
}

/** Dependency warning state - highlights when dependency doesn't exist. */
.dep-warning {
  background-color: var(--n-color-warning-light, rgba(255, 197, 61, 0.1));
  border-radius: 0.25rem;
  padding: 0.25rem;
  margin: -0.25rem;
}

/** Not found indicator for missing dependencies. */
.dep-not-found {
  font-size: 0.75rem;
  color: var(--n-color-warning);
  flex-shrink: 0;
}
</style>
