use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn temp_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("typelua-cli-{}-{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn typelua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_typelua"))
}

fn run(command: &mut Command) -> Output {
    command.output().expect("failed to run typelua")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn compiles_and_runs_a_program() {
    let dir = temp_dir();
    let source = dir.join("hello.tlua");
    let output_path = dir.join("hello.lua");
    fs::write(
        &source,
        "fn greet(str name) -> str { return \"hello, \" + name; }\nprint(greet(\"world\"));\n",
    )
    .unwrap();

    let output = run(typelua().arg(&source));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("hello, world"));

    let generated = fs::read_to_string(&output_path).unwrap();
    assert!(generated.contains("-- tlua:1:1"));
    assert!(generated.contains("function greet"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn check_repairs_multiple_syntax_errors_with_locations() {
    let dir = temp_dir();
    let source = dir.join("broken.tlua");
    fs::write(&source, "int x = ;\nint y = ;\n").unwrap();

    let output = run(typelua().arg("--check").arg(&source));
    assert!(!output.status.success());
    let reported = stderr(&output);
    assert!(reported.contains("line 1, column 9"), "{reported}");
    assert!(reported.contains("line 2, column 9"), "{reported}");
    assert!(reported.contains(&format!("--> {}:1:9", source.display())));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn type_errors_report_line_and_column() {
    let dir = temp_dir();
    let source = dir.join("types.tlua");
    fs::write(&source, "fn f() -> int {\n    int x = 1;\n}\n").unwrap();

    let output = run(typelua().arg("--check").arg(&source));
    assert!(!output.status.success());
    let reported = stderr(&output);
    assert!(reported.contains("must return a value"), "{reported}");
    assert!(reported.contains("line 1, column 1"), "{reported}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compiles_and_runs_classes() {
    let dir = temp_dir();
    let source = dir.join("classes.tlua");
    fs::write(
        &source,
        "class player {\n    int hp = 10;\n    constructor(int hp) { self.hp = hp; }\n    fn alive() -> bool { return self.hp > 0; }\n}\nlocal class Hero = new player(0);\nprint(Hero.alive());\n",
    )
    .unwrap();

    let output = run(typelua().arg(&source));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("false"));
    let generated = fs::read_to_string(dir.join("classes.lua")).unwrap();
    assert!(generated.contains("function player:alive()"));
    assert!(generated.contains("Hero:alive()"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compiles_and_runs_inheritance() {
    let dir = temp_dir();
    let source = dir.join("inheritance.tlua");
    fs::write(
        &source,
        "class Animal {\n    str name = \"animal\";\n    constructor(str name) { self.name = name; }\n    fn speak() -> str { return \"...\"; }\n}\nclass Dog extends Animal {\n    constructor(str name) { super(name); }\n    fn speak() -> str { return \"Woof \" + name; }\n    fn parent_speak() -> str { return super.speak(); }\n}\nclass Cat extends Animal {}\nlocal class pet = new Dog(\"Rex\");\nlocal class cat = new Cat(\"Mia\");\nprint(pet.speak());\nprint(pet.parent_speak());\nprint(cat.name, cat.speak());\n",
    )
    .unwrap();

    let output = run(typelua().arg(&source));
    assert!(output.status.success(), "{}", stderr(&output));
    let printed = stdout(&output);
    assert!(printed.contains("Woof Rex"), "{printed}");
    assert!(printed.contains("..."), "{printed}");
    assert!(printed.contains("Mia"), "{printed}");
    let generated = fs::read_to_string(dir.join("inheritance.lua")).unwrap();
    assert!(generated.contains("Dog = setmetatable({}, { __index = Animal })"));
    assert!(generated.contains("super.__init(self, name)"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn reports_lua_runtime_errors() {
    let dir = temp_dir();
    let source = dir.join("boom.lua");
    fs::write(&source, "error(\"boom\")\n").unwrap();

    let output = run(typelua().arg(&source));
    assert!(!output.status.success());
    assert!(stderr(&output).contains("boom"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn enforces_instruction_limits_from_the_cli() {
    let dir = temp_dir();
    let source = dir.join("loop.lua");
    fs::write(&source, "while true do end\n").unwrap();

    let output = run(typelua()
        .arg("--instruction-limit")
        .arg("20000")
        .arg(&source));
    assert!(!output.status.success());
    assert!(stderr(&output).contains("instruction limit of 20000 exceeded"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn enforces_memory_limits_from_the_cli() {
    let dir = temp_dir();
    let source = dir.join("memory.lua");
    fs::write(
        &source,
        "local chunks = {} for i = 1, 1000 do chunks[i] = string.rep('x', 1024) end\n",
    )
    .unwrap();

    let output = run(typelua().arg("--memory-limit").arg("131072").arg(&source));
    assert!(!output.status.success());
    assert!(stderr(&output).to_lowercase().contains("memory"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn refuses_to_overwrite_handwritten_output() {
    let dir = temp_dir();
    let source = dir.join("main.tlua");
    let output_path = dir.join("main.lua");
    fs::write(&source, "print(\"new\");\n").unwrap();
    fs::write(&output_path, "print(\"old\")\n").unwrap();

    let output = run(typelua().arg(&source));
    assert!(!output.status.success());
    assert!(stderr(&output).contains("was not generated by TypeLua"));
    assert!(fs::read_to_string(&output_path).unwrap().contains("old"));
    let _ = fs::remove_dir_all(dir);
}
