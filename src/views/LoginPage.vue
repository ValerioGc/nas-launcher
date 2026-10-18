<script setup lang="ts">

import { computed, onMounted, reactive, ref, useTemplateRef } from 'vue';
import { t } from '@/i18n';
import { useRouter } from 'vue-router';

import { recoveryOffered } from '@/api/account';
import { usePasswordRecovery } from '@/composables/usePasswordRecovery';
import { useAuthStore } from '@/stores/authStore';
import { useShellStore } from '@/stores/shellStore';
import { validateLogin } from '@/utils/loginValidation';

const auth = useAuthStore();
const router = useRouter();

const username = ref('');
const password = ref('');
const showPassword = ref(false);
const touched = reactive({ username: false, password: false });
const submitError = ref('');
const usernameInput = useTemplateRef<HTMLInputElement>('usernameInput');
const passwordInput = useTemplateRef<HTMLInputElement>('passwordInput');
const recovering = ref(false);
// The reset by mail exists only when the account turned it on and verified its address.
const recoveryAvailable = ref(false);
onMounted(async () => {
  recoveryAvailable.value = await recoveryOffered();
});
const recovery = usePasswordRecovery();
const {
  step: recoveryStep,
  username: recoveryUsername,
  code: recoveryCode,
  password: recoveryPassword,
  repeated: recoveryRepeated,
  busy: recoveryBusy,
  error: recoveryError,
  mismatch: recoveryMismatch,
  ready: recoveryReady,
} = recovery;

/** The reset of the password, starting from the username typed for the sign-in. */
function startRecovery() {
  recovery.restart();
  recoveryUsername.value = username.value.trim();
  recovering.value = true;
}

function endRecovery() {
  if (recoveryUsername.value.trim()) {
    username.value = recoveryUsername.value.trim();
  }
  recovering.value = false;
}

const errors = computed(() => validateLogin(username.value, password.value));
const usernameError = computed(() => (touched.username ? errors.value.username : ''));
const passwordError = computed(() => (touched.password ? errors.value.password : ''));

/**
 * Attempts to log in the user with the provided username and password.
 */
async function login() {
  touched.username = true;
  touched.password = true;
  submitError.value = '';

  const result = await auth.login(username.value, password.value);
  if (!result.success) {
    submitError.value = result.message;
    if (result.code === 'invalid_input') {
      (errors.value.username ? usernameInput : passwordInput).value?.focus();
    }
    return;
  }

  password.value = '';
  showPassword.value = false;
  touched.password = false;
  await useShellStore().endSession();
  await router.push({ name: 'main' });
}

</script>

<template>
  <main class="login_page">
    <section class="login_card" aria-labelledby="login_title">
      <div class="login_heading">
        <h1 id="login_title">NasOS</h1>
      </div>

      <section
        v-if="recovering"
        class="recovery"
        aria-labelledby="recovery_title"
        data-testid="password-recovery"
      >
        <h2 id="recovery_title">{{ t('passwordRecoveryTitle') }}</h2>

        <form v-if="recoveryStep === 'request'" novalidate @submit.prevent="recovery.send">
          <p class="recovery_note">{{ t('passwordRecoveryIntro') }}</p>
          <div class="form_group">
            <label for="recovery_username">{{ t('usernameLabel') }}</label>
            <input
              id="recovery_username"
              v-model="recoveryUsername"
              data-testid="recovery-username"
              type="text"
              autocomplete="username"
              autocapitalize="none"
              :spellcheck="false"
              required
              :readonly="recoveryBusy"
            />
          </div>
          <button
            class="login_button"
            type="submit"
            data-testid="recovery-send"
            :disabled="recoveryBusy || recoveryUsername.trim().length < 3"
          >
            {{ t('passwordRecoverySend') }}
          </button>
        </form>

        <form v-else-if="recoveryStep === 'reset'" novalidate @submit.prevent="recovery.reset">
          <output class="recovery_note">{{ t('passwordRecoverySent') }}</output>
          <div class="form_group">
            <label for="recovery_code">{{ t('passwordRecoveryCode') }}</label>
            <input
              id="recovery_code"
              v-model="recoveryCode"
              data-testid="recovery-code"
              type="text"
              inputmode="numeric"
              autocomplete="one-time-code"
              maxlength="6"
              required
            />
          </div>
          <div class="form_group">
            <label for="recovery_password">{{ t('passwordRecoveryNew') }}</label>
            <input
              id="recovery_password"
              v-model="recoveryPassword"
              data-testid="recovery-password"
              type="password"
              autocomplete="new-password"
              maxlength="128"
              required
            />
          </div>
          <div class="form_group">
            <label for="recovery_repeat">{{ t('passwordRecoveryRepeat') }}</label>
            <input
              id="recovery_repeat"
              v-model="recoveryRepeated"
              data-testid="recovery-repeat"
              type="password"
              autocomplete="new-password"
              maxlength="128"
              required
              :aria-invalid="recoveryMismatch"
            />
            <p v-if="recoveryMismatch" class="field_error">{{ t('passwordRecoveryMismatch') }}</p>
          </div>
          <button
            class="login_button"
            type="submit"
            data-testid="recovery-reset"
            :disabled="recoveryBusy || !recoveryReady"
          >
            {{ t('passwordRecoveryReset') }}
          </button>
        </form>

        <output v-else class="recovery_note" data-testid="recovery-done">
          {{ t('passwordRecoveryDone') }}
        </output>

        <p v-if="recoveryError" role="alert" class="field_error">{{ recoveryError }}</p>
        <button type="button" class="link_button" data-testid="recovery-back" @click="endRecovery">
          {{ t('passwordRecoveryBack') }}
        </button>
      </section>

      <form
        v-else
        id="login_form"
        data-testid="login-form"
        novalidate
        :aria-busy="auth.isLoading"
        @input="submitError = ''"
        @submit.prevent="login"
      >
        <div class="form_group">
          <label for="login_username">{{ t('usernameLabel') }}</label>
          <input
            id="login_username"
            ref="usernameInput"
            v-model="username"
            data-testid="login-username"
            name="username"
            type="text"
            autocomplete="username"
            autocapitalize="none"
            :spellcheck="false"
            :placeholder="t('usernamePlaceholder')"
            required
            :readonly="auth.isLoading"
            :aria-invalid="Boolean(usernameError)"
            :aria-describedby="usernameError ? 'username_error' : undefined"
            @blur="touched.username = true"
          />
          <p v-if="usernameError" id="username_error" class="field_error" aria-live="polite">
            {{ usernameError }}
          </p>
        </div>

        <div class="form_group">
          <label for="login_password">{{ t('passwordLabel') }}</label>
          <div class="password_control">
            <input
              id="login_password"
              ref="passwordInput"
              v-model="password"
              data-testid="login-password"
              name="password"
              :type="showPassword ? 'text' : 'password'"
              autocomplete="current-password"
              :placeholder="t('passwordPlaceholder')"
              required
              :readonly="auth.isLoading"
              :aria-invalid="Boolean(passwordError)"
              :aria-describedby="passwordError ? 'password_error' : undefined"
              @blur="touched.password = true"
            />

            <button
              class="password_toggle"
              type="button"
              :aria-label="showPassword ? t('hidePassword') : t('showPassword')"
              :aria-pressed="showPassword"
              aria-controls="login_password"
              :disabled="auth.isLoading"
              @click="showPassword = !showPassword"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <template v-if="showPassword">
                  <path
                    d="M9.9 5.7A9.4 9.4 0 0 1 12 5.5c5.2 0 9.4 6.5 9.4 6.5a17.3 17.3 0 0 1-3.3 3.8"
                  />
                  <path d="M6.4 7.7A17.3 17.3 0 0 0 2.6 12S6.8 18.5 12 18.5c1.4 0 2.8-.4 4-1" />
                  <path d="M9.8 9.9a3.2 3.2 0 0 0 4.4 4.4" />
                  <path d="m4 4 16 16" />
                </template>
                <template v-else>
                  <path
                    d="M2.6 12S6.8 5.5 12 5.5 21.4 12 21.4 12 17.2 18.5 12 18.5 2.6 12 2.6 12Z"
                  />
                  <circle cx="12" cy="12" r="3.2" />
                </template>
              </svg>
            </button>
          </div>

          <p v-if="passwordError" id="password_error" class="field_error" aria-live="polite">
            {{ passwordError }}
          </p>
        </div>

        <button
          class="login_button"
          data-testid="login-submit"
          type="submit"
          :disabled="auth.isLoading"
        >
          {{ auth.isLoading ? t('signingIn') : t('signIn') }}
        </button>

        <p v-if="submitError" role="alert" class="field_error">{{ submitError }}</p>
        <button
          v-if="recoveryAvailable"
          type="button"
          class="link_button"
          data-testid="login-forgot"
          :disabled="auth.isLoading"
          @click="startRecovery"
        >
          {{ t('forgotPassword') }}
        </button>
      </form>
    </section>
  </main>
</template>

<style lang="scss" scoped>

.login_page {
  --ink: #e8f5ef;
  --ink_muted: #9db3aa;
  --accent: #3fd39b;
  --accent_strong: #5ce3b0;
  --on_accent: #04241a;
  --danger: #ff9d9d;
  --glass: #ffffff14;
  --glass_strong: #ffffff1f;
  --glass_line: #ffffff2b;

  position: relative;
  isolation: isolate;
  min-height: 100vh;
  min-height: 100svh;
  box-sizing: border-box;
  display: grid;
  place-items: center;
  padding: 20px;
  color-scheme: dark;
  color: var(--ink);
  background-color: #04100d;

  font:
    16px / 1.5 system-ui,
    sans-serif;

  * {
    box-sizing: border-box;
  }

  &::before {
    content: '';
    position: fixed;
    inset: 0;
    z-index: -1;
    background-image:
      radial-gradient(32rem 32rem at 12% 16%, #2cc3ae7a, transparent 62%),
      radial-gradient(30rem 30rem at 86% 86%, #1a8fa070, transparent 62%);
    filter: blur(26px);
    animation: drift 26s ease-in-out infinite alternate;
  }
}

p {
  margin: 0;
}

.login_card {
  width: 100%;
  max-width: 400px;
  padding: 28px 24px;
  border: 1px solid var(--glass_line);
  border-radius: 20px;
  background: linear-gradient(155deg, #ffffff1c, #ffffff0a);
  backdrop-filter: blur(24px) saturate(160%);
  -webkit-backdrop-filter: blur(24px) saturate(160%);
  box-shadow:
    0 24px 60px #00000059,
    inset 0 1px 0 #ffffff2e;

  .login_heading {
    margin-bottom: 24px;

    h1 {
      margin: auto 0 8px;
      font-size: 26px;
      line-height: 1.2;
      letter-spacing: -0.02em;
      text-align: center;
    }
  }
}

// Background drift animation ****************************
@keyframes drift {
  from {
    transform: translate3d(-1.5%, -1%, 0) scale(1);
  }

  to {
    transform: translate3d(1.5%, 1.5%, 0) scale(1.06);
  }
}

@media (prefers-reduced-motion: reduce) {
  .login_page::before {
    animation: none;
  }
}

// Input styles ****************************
.form_group {
  margin-bottom: 16px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 14px;
    color: var(--ink_muted);
  }

  input {
    width: 100%;
    min-width: 0;
    background: var(--glass);
    color: var(--ink);

    &::placeholder {
      color: var(--ink_muted);
    }

    &[aria-invalid='true'] {
      border-color: var(--danger);
    }
  }

  .password_control {
    position: relative;

    input {
      padding-right: 46px;
    }
  }
}

input {
  min-height: 46px;
  padding: 10px 14px;
  border: 1px solid var(--glass_line);
  border-radius: 12px;
  font: inherit;
  transition:
    border-color 160ms ease,
    box-shadow 160ms ease;

  &:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px #3fd39b3d;
  }
}

// Button styles ****************************
button {
  cursor: pointer;
  font: inherit;

  &:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  &:disabled {
    cursor: wait;
    opacity: 0.65;
  }
}

.password_toggle {
  position: absolute;
  top: 50%;
  right: 6px;
  transform: translateY(-50%);
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--ink_muted);
  transition:
    color 160ms ease,
    background 160ms ease;

  &:hover:not(:disabled) {
    background: var(--glass_strong);
    color: var(--ink);
  }

  &[aria-pressed='true'] {
    color: var(--accent);
  }

  svg {
    width: 20px;
    height: 20px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
}

.login_button {
  width: 100%;
  min-height: 46px;
  margin-top: 4px;
  padding: 10px 14px;
  border: 0;
  border-radius: 12px;
  background: linear-gradient(135deg, var(--accent_strong), var(--accent));
  color: var(--on_accent);
  font-weight: 650;
  box-shadow: 0 8px 20px #3fd39b33;
  transition:
    filter 160ms ease,
    box-shadow 160ms ease;

  &:hover:not(:disabled) {
    filter: brightness(1.08);
    box-shadow: 0 10px 26px #3fd39b47;
  }
}

// Password recovery ****************************
.recovery h2 {
  margin: 0 0 8px;
  font-size: 18px;
}

.recovery_note {
  display: block;
  margin: 0 0 16px;
  color: var(--ink_muted);
  font-size: 14px;
}

.link_button {
  display: block;
  margin: 14px auto 0;
  padding: 4px 8px;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-size: 14px;
  text-decoration: underline;
}

// Validation ****************************
.field_error {
  margin-block: 8px;
  font-size: 14px;
  overflow-wrap: anywhere;
  color: var(--danger);
}

</style>
