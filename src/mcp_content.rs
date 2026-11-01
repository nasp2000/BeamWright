//! Resources and prompts offered over MCP.
//!
//! Resources are documents an agent can read: short guides on how to work
//! safely, plus a live summary of the open project. Prompts are ready-made
//! instructions that fill in the material or quantity the agent was asked for.

use serde_json::{Value, json};

use crate::document::Document;

/// Upper bound for a prompt argument, so a model cannot paste a file in.
const MAX_ARGUMENT: usize = 2000;

/// The resources advertised by `resources/list`.
pub fn resources() -> Vec<Value> {
    vec![
        json!({
            "uri": "beamwright://guide/workflow",
            "name": "Workflow",
            "description": "How to inspect, change, check and export a project safely.",
            "mimeType": "text/markdown",
        }),
        json!({
            "uri": "beamwright://guide/coordinates",
            "name": "Coordinates and units",
            "description": "Page origin, millimetres and how bounds are reported.",
            "mimeType": "text/markdown",
        }),
        json!({
            "uri": "beamwright://document/summary",
            "name": "Current document",
            "description": "Live structure of the open project as JSON.",
            "mimeType": "application/json",
        }),
        json!({
            "uri": "beamwright://document/parameters",
            "name": "Parameters",
            "description": "Named values and the expressions that use them, as JSON.",
            "mimeType": "application/json",
        }),
    ]
}

const WORKFLOW: &str = "\
# Working on a BeamWright project

BeamWright prepares files. It never connects to, configures or drives a machine.

1. Call `inspect` to see the layers, objects, operations and parameters. Add
   `ids` when you only care about part of the drawing.
2. Call `measure` before moving things: it reports path length, area, bounds and
   the cut/travel distance of the job.
3. Make changes with `apply_batch`: steps are all-or-nothing and become one undo
   entry. A failed batch leaves the document exactly as it was and names the
   step that failed.
4. Use `checkpoint` before a risky edit and `restore_checkpoint` to go back.
5. Call `check_job` before exporting. Errors mean the file would be empty or
   invalid; warnings deserve a look but do not block the export.
6. Export with `export_svg`, `export_dxf` or `export_gcode` only when the report
   has no errors. The user loads the file into the sender of their choice.

Never promise a physical result: speed, power and duration are predictions from
the profile and material, not guarantees for any equipment.
";

const COORDINATES: &str = "\
# Coordinates and units

- All geometry is stored in millimetres. The page origin is the bottom-left
  corner unless the project says otherwise; `inspect` reports `origin`.
- The page spans `(0, 0)` to `(width_mm, height_mm)`. Y grows upwards.
- Object transforms are separate from the shape: `world_path()` (used by
  `measure` and `inspect`) already applies them.
- `bounds_mm` is the axis-aligned box of the measured objects, in millimetres.
- `length_mm` is the outline length; `area_mm2` uses the flattened outline and
  is zero for open paths. `net_area_mm2` subtracts the area of holes.
- Layers have visibility and lock flags; hidden layers are left out of jobs.
- Operations carry speed in mm/min, power in percent and the number of passes.
  Expressions may override ten numeric fields, evaluated against the document
  parameters.
";

/// Read one resource for the current document.
pub fn read(uri: &str, document: &Document) -> Result<Value, String> {
    let (mime, text) = match uri {
        "beamwright://guide/workflow" => ("text/markdown", WORKFLOW.to_string()),
        "beamwright://guide/coordinates" => ("text/markdown", COORDINATES.to_string()),
        "beamwright://document/summary" => (
            "application/json",
            json!(crate::inspection::inspect(document, None)).to_string(),
        ),
        "beamwright://document/parameters" => (
            "application/json",
            json!({
                "parameters": crate::parameters::resolved(&document.parameters)
                    .into_iter()
                    .zip(&document.parameters)
                    .map(|((name, value), parameter)| match value {
                        Ok(value) => json!({
                            "name": name,
                            "expression": parameter.expression,
                            "unit": parameter.unit,
                            "value": value,
                        }),
                        Err(error) => json!({
                            "name": name,
                            "expression": parameter.expression,
                            "unit": parameter.unit,
                            "error": error,
                        }),
                    })
                    .collect::<Vec<_>>(),
                "operations": document
                    .operations
                    .iter()
                    .filter(|operation| !operation.params.expressions.is_empty())
                    .map(|operation| json!({
                        "id": operation.id,
                        "name": operation.name,
                        "expressions": operation.params.expressions,
                    }))
                    .collect::<Vec<_>>(),
            })
            .to_string(),
        ),
        other => return Err(format!("unknown resource: {other}")),
    };
    Ok(json!({"uri": uri, "mimeType": mime, "text": text}))
}

/// The prompts advertised by `prompts/list`.
pub fn prompts() -> Vec<Value> {
    vec![
        json!({
            "name": "prepare_job",
            "description": "Inspect the project, measure it and propose a checked batch for a material.",
            "arguments": [
                {"name": "material", "description": "Material and thickness, e.g. 3 mm plywood", "required": false},
                {"name": "notes", "description": "Anything the user asked for", "required": false},
            ],
        }),
        json!({
            "name": "fix_preflight",
            "description": "Run the preflight report and fix its errors one at a time.",
        }),
        json!({
            "name": "outline_and_nest",
            "description": "Convert text to outlines and arrange copies inside the stock.",
            "arguments": [
                {"name": "quantity", "description": "How many copies to place", "required": false},
                {"name": "gap_mm", "description": "Space between parts in millimetres", "required": false},
            ],
        }),
        json!({
            "name": "measure_selection",
            "description": "Measure the current selection and explain the numbers.",
        }),
    ]
}

/// Build one prompt message, filling in the arguments.
pub fn prompt(name: &str, arguments: &Value) -> Result<Value, String> {
    let argument = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(MAX_ARGUMENT).collect::<String>())
    };
    let (description, text) = match name {
        "prepare_job" => {
            let material =
                argument("material").unwrap_or_else(|| "the material the user names".to_string());
            let notes = argument("notes").unwrap_or_else(|| "none".to_string());
            (
                "Prepare and check a laser job",
                format!(
                    "Prepare a laser job for {material}.\n\n\
1. Call `inspect` and `measure` and sum up the project.\n\
2. Call `check_job`; fix errors and explain worthwhile warnings.\n\
3. Propose an `apply_batch` plan for the remaining changes, one step per tool.\n\
4. Wait for confirmation before exporting. Use `export_gcode` for the job file\n\
   and name the file the user asked for.\n\n\
User notes: {notes}"
                ),
            )
        }
        "fix_preflight" => (
            "Fix the preflight report",
            "Call `check_job`. Explain every error and warning with its code and \
object id. Fix one error at a time with `apply_batch`, then call `check_job` \
again. Do not export while errors remain, and tell the user which warnings are \
still present when the report is clean."
                .to_string(),
        ),
        "outline_and_nest" => {
            let quantity =
                argument("quantity").unwrap_or_else(|| "the number the user asks".to_string());
            let gap = argument("gap_mm").unwrap_or_else(|| "the spacing the user asks".to_string());
            (
                "Outline text and nest copies",
                format!(
                    "Convert every text object to outlines with `text_to_outlines`, then \
nest {quantity} copies with a {gap} mm gap using `nest`. Measure the result with \
`measure` and check it with `check_job` before answering."
                ),
            )
        }
        "measure_selection" => (
            "Measure the selection",
            "Call `measure` for the current selection and explain the length, area, \
bounds and, when present, the cut and travel distance of the job. Mention that \
duration and results are estimates for the chosen material and profile."
                .to_string(),
        ),
        other => return Err(format!("unknown prompt: {other}")),
    };
    Ok(json!({
        "description": description,
        "messages": [{
            "role": "user",
            "content": {"type": "text", "text": text}
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::{prompt, prompts, read, resources};
    use crate::document::Document;

    #[test]
    fn resources_are_advertised_and_readable() {
        let listed = resources();
        assert_eq!(listed.len(), 4);
        let document = Document::default();
        for resource in &listed {
            let uri = resource["uri"].as_str().unwrap();
            let content = read(uri, &document).unwrap();
            assert_eq!(content["uri"], *uri);
            assert_ne!(content["text"].as_str().unwrap(), "");
        }
        assert!(read("beamwright://missing", &document).is_err());
    }

    #[test]
    fn the_document_resource_reflects_the_project() {
        let mut document = Document::new("Lamp", 200.0, 100.0);
        document.parameters = vec![crate::parameters::Parameter::new("feed", "900")];
        let summary = read("beamwright://document/summary", &document).unwrap();
        let summary: serde_json::Value =
            serde_json::from_str(summary["text"].as_str().unwrap()).unwrap();
        assert_eq!(summary["project"]["name"], "Lamp");
        let parameters = read("beamwright://document/parameters", &document).unwrap();
        let parameters: serde_json::Value =
            serde_json::from_str(parameters["text"].as_str().unwrap()).unwrap();
        assert_eq!(parameters["parameters"][0]["value"], 900.0);
    }

    #[test]
    fn prompts_list_and_fill_their_arguments() {
        let listed = prompts();
        assert_eq!(listed.len(), 4);
        let message = prompt(
            "prepare_job",
            &serde_json::json!({"material": "3 mm plywood"}),
        )
        .unwrap();
        let text = message["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("3 mm plywood"));
        assert!(text.contains("check_job"));
        let message = prompt("outline_and_nest", &serde_json::json!({"quantity": "12"})).unwrap();
        assert!(
            message["messages"][0]["content"]["text"]
                .as_str()
                .unwrap()
                .contains("12")
        );
        assert!(prompt("missing", &serde_json::json!({})).is_err());
    }

    #[test]
    fn long_arguments_are_truncated() {
        let long = "x".repeat(super::MAX_ARGUMENT + 50);
        let message = prompt("prepare_job", &serde_json::json!({"notes": long})).unwrap();
        let text = message["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.len() < super::MAX_ARGUMENT + 600);
        assert!(!text.contains(&"x".repeat(super::MAX_ARGUMENT + 1)));
    }
}
