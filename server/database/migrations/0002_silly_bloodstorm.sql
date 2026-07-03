CREATE TYPE "public"."meal_plan_recipe_status" AS ENUM('suggested', 'accepted', 'swapped', 'cooked');--> statement-breakpoint
CREATE TYPE "public"."meal_plan_status" AS ENUM('draft', 'validated', 'cooked');--> statement-breakpoint
CREATE TABLE "meal_plan_recipes" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"meal_plan_id" uuid NOT NULL,
	"recipe_id" uuid NOT NULL,
	"position" integer NOT NULL,
	"target_servings" integer NOT NULL,
	"status" "meal_plan_recipe_status" DEFAULT 'suggested' NOT NULL,
	"cooked_at" timestamp with time zone
);
--> statement-breakpoint
CREATE TABLE "meal_plans" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"week_start_date" date NOT NULL,
	"people_count" integer NOT NULL,
	"dish_count" integer NOT NULL,
	"days_count" integer NOT NULL,
	"total_servings" integer NOT NULL,
	"status" "meal_plan_status" DEFAULT 'draft' NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "default_dish_count" integer DEFAULT 4 NOT NULL;--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "default_people_count" integer DEFAULT 1 NOT NULL;--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "default_days_count" integer DEFAULT 7 NOT NULL;--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "rotation_weeks" integer DEFAULT 3 NOT NULL;--> statement-breakpoint
ALTER TABLE "meal_plan_recipes" ADD CONSTRAINT "meal_plan_recipes_meal_plan_id_meal_plans_id_fk" FOREIGN KEY ("meal_plan_id") REFERENCES "public"."meal_plans"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "meal_plan_recipes" ADD CONSTRAINT "meal_plan_recipes_recipe_id_recipes_id_fk" FOREIGN KEY ("recipe_id") REFERENCES "public"."recipes"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "meal_plans" ADD CONSTRAINT "meal_plans_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "meal_plan_recipes_meal_plan_id_idx" ON "meal_plan_recipes" USING btree ("meal_plan_id");--> statement-breakpoint
CREATE INDEX "meal_plan_recipes_recipe_id_idx" ON "meal_plan_recipes" USING btree ("recipe_id");--> statement-breakpoint
CREATE UNIQUE INDEX "meal_plans_user_id_week_idx" ON "meal_plans" USING btree ("user_id","week_start_date");