/**
 * How an operation is going. Only `running` and `paused` can still be asked for anything.
 */
export type TaskState = 'running' | 'paused' | 'done' | 'failed' | 'cancelled';

/**
 * One ongoing operation, as the register keeps it.
 * `progress` is `null` when whoever is working cannot tell how far along it is, and `pausable`
 * and `cancellable` say what its owner agreed to be asked for.
 */
export interface UserTask {
  id: string;
  source: string;
  name: string;
  detail: string;
  progress: number | null;
  state: TaskState;
  pausable: boolean;
  cancellable: boolean;
  createdAt: number;
  updatedAt: number;
}
