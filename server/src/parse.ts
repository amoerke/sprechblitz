// Strukturiertes Parsen von Transkripten (Workout / Mahlzeit) über OpenAI Structured Outputs.
// Wird von der kraft.-App über deren Supabase Edge Function aufgerufen.

export class ParseUpstreamError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string
  ) {
    super(`OpenAI parse failed with status ${status}`);
  }
}

export interface ParseOptions {
  apiKey: string;
  baseUrl: string;
  model: string;
  text: string;
  /** ISO timestamp of the request, used for meal-type defaults */
  now: string;
}

const NUMBER_OR_NULL = { type: ["number", "null"] };

const workoutSchema = {
  name: "workout",
  strict: true,
  schema: {
    type: "object",
    additionalProperties: false,
    properties: {
      title: { type: "string", description: "Kurzer deutscher Titel, z. B. 'Kettlebell Ganzkörper'" },
      durationMin: { ...NUMBER_OR_NULL, description: "Nur wenn eine Gesamtdauer genannt wurde" },
      muscleGroups: {
        type: "array",
        items: { type: "string" },
        description: "1–4 deutsche Muskelgruppen wie 'Beine', 'Rücken', 'Core', 'Brust', 'Schultern', 'Arme', 'Ganzkörper', 'Griffkraft'",
      },
      exercises: {
        type: "array",
        items: {
          type: "object",
          additionalProperties: false,
          properties: {
            name: { type: "string", description: "Übungsname wie üblich geschrieben, z. B. 'Goblet Squats'" },
            sets: { type: "integer" },
            reps: { ...NUMBER_OR_NULL, description: "Wiederholungen pro Satz, null bei Strecken-Übungen" },
            perSide: { type: "boolean", description: "true bei 'pro Seite' / 'je Seite'" },
            weightKg: { ...NUMBER_OR_NULL, description: "Gewicht in kg pro Hand/Gerät, null bei Körpergewicht" },
            distanceM: { ...NUMBER_OR_NULL, description: "Strecke in Metern, z. B. Farmer's Carry" },
            bodyweight: { type: "boolean", description: "true wenn ausdrücklich ohne Zusatzgewicht" },
            unclear: {
              type: ["object", "null"],
              additionalProperties: false,
              description: "Nur wenn das Transkript mehrdeutig ist (z. B. 'zweiunddreißig oder zweiundzwanzig')",
              properties: {
                question: { type: "string", description: "Kurze Rückfrage auf Deutsch mit dem Originalwortlaut" },
                field: { type: "string", enum: ["weightKg", "reps", "sets"] },
                options: { type: "array", items: { type: "number" }, description: "2–3 Kandidaten, wahrscheinlichster zuletzt" },
              },
              required: ["question", "field", "options"],
            },
          },
          required: ["name", "sets", "reps", "perSide", "weightKg", "distanceM", "bodyweight", "unclear"],
        },
      },
    },
    required: ["title", "durationMin", "muscleGroups", "exercises"],
  },
} as const;

const mealSchema = {
  name: "meal",
  strict: true,
  schema: {
    type: "object",
    additionalProperties: false,
    properties: {
      name: { type: "string", description: "Kurzer deutscher Name der Mahlzeit, z. B. 'Proteinshake mit Banane'" },
      kcal: { type: "integer" },
      proteinG: { type: "number" },
      carbsG: { type: "number" },
      fatG: { type: "number" },
      mealType: { type: "string", enum: ["fruehstueck", "mittag", "abendbrot", "snack"] },
    },
    required: ["name", "kcal", "proteinG", "carbsG", "fatG", "mealType"],
  },
} as const;

const WORKOUT_PROMPT = `Du bist ein Trainings-Logbuch. Du bekommst das deutsche Transkript einer Sprachnotiz nach einem Workout (functional Training, Kettlebell, Freeletics, Hyrox u. ä.) und erzeugst daraus strukturierte Daten.
Regeln:
- Zahlwörter in Zahlen umwandeln ("vierundzwanzig Kilo" → 24).
- "drei Runden", "drei Sätze", "3 mal" → sets. "je zwölf", "zwölf Wiederholungen" → reps.
- "pro Seite", "je Seite", "pro Arm" → perSide = true.
- Strecken ("vierzig Meter") → distanceM, reps = null.
- Ohne Gewichtsangabe bei Körpergewichtsübungen (Burpees, Pull-ups, Push-ups, Dips, Sit-ups) → weightKg = null, bodyweight = true.
- Wenn der Sprecher sich korrigiert ("nein, zweiunddreißig"), gilt die Korrektur.
- Wenn zwei Werte konkurrieren oder das Transkript hörbar unsicher ist ("zweiunddreißig oder zweiundzwanzig"), setze den wahrscheinlicheren Wert und fülle "unclear" mit Rückfrage und Kandidaten. Sonst unclear = null.
- Erfinde keine Übungen. Reihenfolge wie gesprochen.
- Titel: 2–4 Wörter, deutsch, beschreibt Gerät/Schwerpunkt. Muskelgruppen aus den Übungen ableiten.
- durationMin nur, wenn eine Gesamtdauer genannt wird ("achtundvierzig Minuten insgesamt").`;

const MEAL_PROMPT = `Du bist ein Ernährungs-Logbuch. Du bekommst das deutsche Transkript einer Sprachnotiz zu einer Mahlzeit und schätzt Kalorien und Makros realistisch für übliche Portionsgrößen in Deutschland.
Regeln:
- Mengenangaben ("zweihundert Gramm", "zwei Scheiben") berücksichtigen; ohne Angabe eine normale Portion annehmen.
- Werte ganzzahlig runden (kcal) bzw. auf ganze Gramm.
- mealType aus dem Text ableiten ("zum Frühstück", "mittags"); sonst aus der Uhrzeit: vor 10 Uhr fruehstueck, 10–15 mittag, 15–17 snack, danach abendbrot.
- Name kurz und konkret, ohne Mengen.`;

async function complete<T>(options: ParseOptions, system: string, schema: object): Promise<T> {
  const response = await fetch(`${options.baseUrl}/chat/completions`, {
    method: "POST",
    headers: { Authorization: `Bearer ${options.apiKey}`, "Content-Type": "application/json" },
    body: JSON.stringify({
      model: options.model,
      temperature: 0,
      messages: [
        { role: "system", content: system },
        { role: "user", content: `Aktuelle Zeit: ${options.now}\n\nTranskript:\n${options.text}` },
      ],
      response_format: { type: "json_schema", json_schema: schema },
    }),
  });
  if (!response.ok) {
    throw new ParseUpstreamError(response.status, await response.text());
  }
  const result = (await response.json()) as { choices?: { message?: { content?: string; refusal?: string } }[] };
  const message = result.choices?.[0]?.message;
  if (!message?.content) {
    throw new ParseUpstreamError(502, message?.refusal ?? "empty completion");
  }
  return JSON.parse(message.content) as T;
}

export interface ParsedExercise {
  name: string;
  sets: number;
  reps: number | null;
  perSide: boolean;
  weightKg: number | null;
  distanceM: number | null;
  bodyweight: boolean;
  unclear: { question: string; field: "weightKg" | "reps" | "sets"; options: number[] } | null;
}

export interface ParsedWorkout {
  title: string;
  durationMin: number | null;
  muscleGroups: string[];
  exercises: ParsedExercise[];
}

export interface ParsedMeal {
  name: string;
  kcal: number;
  proteinG: number;
  carbsG: number;
  fatG: number;
  mealType: "fruehstueck" | "mittag" | "abendbrot" | "snack";
}

export function parseWorkout(options: ParseOptions): Promise<ParsedWorkout> {
  return complete<ParsedWorkout>(options, WORKOUT_PROMPT, workoutSchema);
}

export function parseMeal(options: ParseOptions): Promise<ParsedMeal> {
  return complete<ParsedMeal>(options, MEAL_PROMPT, mealSchema);
}
