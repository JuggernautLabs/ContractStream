// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { Index } from "./Index";
import type { Job } from "./Job";
import type { Proposal } from "./Proposal";
import type { User } from "./User";

export interface PendingJob { job_id: Index<Job>, user_id: Index<User>, proposal_id: Index<Proposal>, }