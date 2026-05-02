// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';

import { ApiError, apiClient, type SignInResponse } from '../../lib/api';
import { Brandmark } from './Brandmark';

const signInSchema = z.object({
  email: z
    .string()
    .min(1, 'E-Mail erforderlich')
    .email('Ungültige E-Mail-Adresse'),
  password: z.string().min(1, 'Passwort erforderlich'),
});
type SignInValues = z.infer<typeof signInSchema>;

const newPwSchema = z
  .object({
    new_password: z
      .string()
      .min(12, 'Mindestens 12 Zeichen')
      .regex(/[a-z]/, 'Mindestens ein Kleinbuchstabe')
      .regex(/[A-Z]/, 'Mindestens ein Großbuchstabe')
      .regex(/[0-9]/, 'Mindestens eine Ziffer'),
    confirm: z.string(),
  })
  .refine((v) => v.new_password === v.confirm, {
    path: ['confirm'],
    message: 'Passwörter stimmen nicht überein',
  });
type NewPwValues = z.infer<typeof newPwSchema>;

type Stage =
  | { kind: 'sign_in' }
  | { kind: 'new_password'; session: string; email: string };

export default function LoginForm() {
  const [stage, setStage] = useState<Stage>({ kind: 'sign_in' });
  const [serverError, setServerError] = useState<string | null>(null);

  const handleAuthResponse = (res: SignInResponse) => {
    if (res.result === 'authenticated') {
      window.location.href = '/competitions';
      return;
    }
    setStage({ kind: 'new_password', session: res.session, email: res.email });
  };

  return (
    <div className="flex w-full max-w-md flex-col items-center gap-10">
      <div className="flex flex-col items-center gap-5">
        <Brandmark size="lg" />
        <div className="flex flex-col items-center gap-1">
          <span className="font-stencil text-[44px] font-bold leading-none tracking-tight text-ink-50">
            KI-DO TIMER
          </span>
          <span className="label">Wettkampf-Kontrollstand</span>
        </div>
      </div>

      <div className="w-full">
        {stage.kind === 'sign_in' ? (
          <SignInStep
            onResult={handleAuthResponse}
            onError={setServerError}
            error={serverError}
          />
        ) : (
          <NewPasswordStep
            email={stage.email}
            session={stage.session}
            onResult={handleAuthResponse}
            onError={setServerError}
            error={serverError}
          />
        )}
      </div>
    </div>
  );
}

function SignInStep({
  onResult,
  onError,
  error,
}: {
  onResult: (res: SignInResponse) => void;
  onError: (msg: string | null) => void;
  error: string | null;
}) {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<SignInValues>({
    resolver: zodResolver(signInSchema),
    defaultValues: { email: '', password: '' },
  });

  const onSubmit = async (values: SignInValues) => {
    onError(null);
    try {
      const res = await apiClient.auth.signIn({
        email: values.email.trim().toLowerCase(),
        password: values.password,
      });
      onResult(res);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        onError('E-Mail oder Passwort falsch.');
        return;
      }
      onError(err instanceof Error ? err.message : 'Anmeldung fehlgeschlagen');
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
      <div className="field">
        <label className="label" htmlFor="email">
          E-Mail-Adresse
        </label>
        <input
          id="email"
          type="email"
          autoComplete="email"
          autoFocus
          className="input"
          {...register('email')}
        />
        {errors.email && (
          <span className="text-[11px] text-status-unknown">{errors.email.message}</span>
        )}
      </div>

      <div className="field">
        <label className="label" htmlFor="password">
          Passwort
        </label>
        <input
          id="password"
          type="password"
          autoComplete="current-password"
          className="input"
          {...register('password')}
        />
        {errors.password && (
          <span className="text-[11px] text-status-unknown">{errors.password.message}</span>
        )}
      </div>

      {error && (
        <p className="rounded-md bg-status-unknown/10 px-3 py-2 text-[12px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      <button type="submit" className="btn btn-primary" disabled={isSubmitting}>
        {isSubmitting ? 'Wird angemeldet …' : 'Anmelden'}
      </button>

      <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-ink-300 text-center">
        Authentifizierung über AWS Cognito
      </p>
    </form>
  );
}

function NewPasswordStep({
  email,
  session,
  onResult,
  onError,
  error,
}: {
  email: string;
  session: string;
  onResult: (res: SignInResponse) => void;
  onError: (msg: string | null) => void;
  error: string | null;
}) {
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<NewPwValues>({
    resolver: zodResolver(newPwSchema),
    defaultValues: { new_password: '', confirm: '' },
  });

  const onSubmit = async (values: NewPwValues) => {
    onError(null);
    try {
      const res = await apiClient.auth.completeNewPassword({
        email,
        new_password: values.new_password,
        session,
      });
      onResult(res);
    } catch (err) {
      onError(err instanceof Error ? err.message : 'Passwortänderung fehlgeschlagen');
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4" noValidate>
      <div className="rounded-lg border border-[color:var(--hairline-color)] bg-status-armed/10 px-4 py-3">
        <span className="label text-status-armed">Neues Passwort erforderlich</span>
        <p className="mt-1 text-[12px] text-ink-100">
          {email}: das temporäre Passwort muss jetzt ersetzt werden.
        </p>
      </div>

      <div className="field">
        <label className="label" htmlFor="new_password">
          Neues Passwort
        </label>
        <input
          id="new_password"
          type="password"
          autoComplete="new-password"
          autoFocus
          className="input"
          {...register('new_password')}
        />
        {errors.new_password && (
          <span className="text-[11px] text-status-unknown">{errors.new_password.message}</span>
        )}
        <span className="font-mono text-[10px] text-ink-300">
          ≥ 12 Zeichen, Groß-/Kleinbuchstaben, Ziffern
        </span>
      </div>

      <div className="field">
        <label className="label" htmlFor="confirm">
          Wiederholen
        </label>
        <input
          id="confirm"
          type="password"
          autoComplete="new-password"
          className="input"
          {...register('confirm')}
        />
        {errors.confirm && (
          <span className="text-[11px] text-status-unknown">{errors.confirm.message}</span>
        )}
      </div>

      {error && (
        <p className="rounded-md bg-status-unknown/10 px-3 py-2 text-[12px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      <button type="submit" className="btn btn-primary" disabled={isSubmitting}>
        {isSubmitting ? 'Wird gespeichert …' : 'Passwort setzen'}
      </button>
    </form>
  );
}
