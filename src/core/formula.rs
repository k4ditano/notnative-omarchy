//! Sistema de fórmulas tipo Excel para Bases
//! 
//! Soporta:
//! - Referencias de celdas: A1, B2, AA1
//! - Rangos: A1:C10, B:B (columna entera)
//! - Funciones: SUM, AVG, MIN, MAX, COUNT, IF
//! - Operadores: + - * / ( )
//! - Comparadores: > < = >= <= <>

use std::collections::HashMap;
use std::fmt;

/// Referencia a una celda (columna + fila)
/// Las columnas van de A-ZZ (0-701), las filas de 1-N
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellRef {
    /// Índice de columna (0 = A, 1 = B, ..., 25 = Z, 26 = AA, ...)
    pub col: u16,
    /// Número de fila (1-indexed como Excel)
    pub row: u32,
}

impl CellRef {
    /// Crear una nueva referencia de celda
    pub fn new(col: u16, row: u32) -> Self {
        Self { col, row }
    }

    /// Parsear una referencia de celda desde string (ej: "A1", "BC42")
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_uppercase();
        
        // Encontrar dónde terminan las letras y empiezan los números
        let mut col_end = 0;
        for (i, c) in s.char_indices() {
            if c.is_ascii_alphabetic() {
                col_end = i + 1;
            } else {
                break;
            }
        }
        
        if col_end == 0 || col_end >= s.len() {
            return None;
        }
        
        let col_str = &s[..col_end];
        let row_str = &s[col_end..];
        
        let col = col_from_letters(col_str)?;
        let row: u32 = row_str.parse().ok()?;
        
        if row == 0 {
            return None; // Filas empiezan en 1
        }
        
        Some(Self { col, row })
    }

    /// Convertir a string (ej: "A1", "BC42")
    pub fn to_string(&self) -> String {
        format!("{}{}", col_to_letters(self.col), self.row)
    }
}

impl fmt::Display for CellRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", col_to_letters(self.col), self.row)
    }
}

/// Rango de celdas
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellRange {
    /// Rango rectangular (A1:C10)
    Range { start: CellRef, end: CellRef },
    /// Columna entera (B:B)
    Column { col: u16 },
    /// Fila entera (3:3)
    Row { row: u32 },
    /// Celda individual
    Single(CellRef),
}

impl CellRange {
    /// Parsear un rango desde string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_uppercase();
        
        if let Some(colon_pos) = s.find(':') {
            let left = &s[..colon_pos];
            let right = &s[colon_pos + 1..];
            
            // Columna entera (B:B)
            if left.chars().all(|c| c.is_ascii_alphabetic()) && right.chars().all(|c| c.is_ascii_alphabetic()) {
                let col = col_from_letters(left)?;
                return Some(CellRange::Column { col });
            }
            
            // Fila entera (3:3)
            if let (Ok(row1), Ok(row2)) = (left.parse::<u32>(), right.parse::<u32>()) {
                if row1 == row2 {
                    return Some(CellRange::Row { row: row1 });
                }
            }
            
            // Rango rectangular (A1:C10)
            let start = CellRef::parse(left)?;
            let end = CellRef::parse(right)?;
            return Some(CellRange::Range { start, end });
        }
        
        // Celda individual
        CellRef::parse(&s).map(CellRange::Single)
    }

    /// Obtener todas las celdas en el rango (con límite de filas para columnas enteras)
    pub fn cells(&self, max_rows: u32) -> Vec<CellRef> {
        match self {
            CellRange::Single(cell) => vec![*cell],
            CellRange::Range { start, end } => {
                let mut cells = Vec::new();
                let col_start = start.col.min(end.col);
                let col_end = start.col.max(end.col);
                let row_start = start.row.min(end.row);
                let row_end = start.row.max(end.row);
                
                for col in col_start..=col_end {
                    for row in row_start..=row_end {
                        cells.push(CellRef::new(col, row));
                    }
                }
                cells
            }
            CellRange::Column { col } => {
                (1..=max_rows).map(|row| CellRef::new(*col, row)).collect()
            }
            CellRange::Row { row } => {
                // Asumimos un máximo razonable de columnas
                (0..26u16).map(|col| CellRef::new(col, *row)).collect()
            }
        }
    }
}

/// Convertir índice de columna a letras (0 = A, 25 = Z, 26 = AA, ...)
pub fn col_to_letters(col: u16) -> String {
    let mut result = String::new();
    let mut n = col as i32 + 1; // 1-indexed para el cálculo
    
    while n > 0 {
        n -= 1;
        let remainder = (n % 26) as u8;
        result.insert(0, (b'A' + remainder) as char);
        n /= 26;
    }
    
    result
}

/// Convertir letras a índice de columna (A = 0, Z = 25, AA = 26, ...)
pub fn col_from_letters(s: &str) -> Option<u16> {
    let mut result: u32 = 0;
    
    for c in s.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        let val = (c.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        result = result * 26 + val;
    }
    
    if result == 0 {
        return None;
    }
    
    Some((result - 1) as u16)
}

// ============================================================================
// TOKENS Y PARSER
// ============================================================================

/// Token de fórmula
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    CellRef(CellRef),
    Range(CellRange),
    Function(String),
    Plus,
    Minus,
    Multiply,
    Divide,
    LParen,
    RParen,
    Comma,
    Equals,
    NotEquals,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    String(String),
}

/// Tokenizar una fórmula
pub fn tokenize(formula: &str) -> Result<Vec<Token>, FormulaError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = formula.chars().collect();
    let mut i = 0;
    
    // Saltar el '=' inicial si existe
    if chars.first() == Some(&'=') {
        i = 1;
    }
    
    while i < chars.len() {
        let c = chars[i];
        
        // Espacios
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        
        // Números
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            let num: f64 = num_str.parse().map_err(|_| FormulaError::InvalidNumber(num_str))?;
            tokens.push(Token::Number(num));
            continue;
        }
        
        // Strings entre comillas
        if c == '"' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            tokens.push(Token::String(s));
            if i < chars.len() {
                i += 1; // Saltar comilla de cierre
            }
            continue;
        }
        
        // Identificadores (funciones o referencias de celda)
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == ':' || chars[i] == '$') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            let ident_upper = ident.to_uppercase();
            
            // ¿Es una función? (seguida de paréntesis)
            let is_function = i < chars.len() && chars[i] == '(';
            
            if is_function {
                tokens.push(Token::Function(ident_upper));
            } else if let Some(range) = CellRange::parse(&ident_upper) {
                match range {
                    CellRange::Single(cell) => tokens.push(Token::CellRef(cell)),
                    _ => tokens.push(Token::Range(range)),
                }
            } else {
                return Err(FormulaError::InvalidIdentifier(ident));
            }
            continue;
        }
        
        // Operadores
        match c {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Multiply),
            '/' => tokens.push(Token::Divide),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '=' => tokens.push(Token::Equals),
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::GreaterEq);
                    i += 1;
                } else {
                    tokens.push(Token::Greater);
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::LessEq);
                    i += 1;
                } else if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::NotEquals);
                    i += 1;
                } else {
                    tokens.push(Token::Less);
                }
            }
            _ => return Err(FormulaError::UnexpectedChar(c)),
        }
        i += 1;
    }
    
    Ok(tokens)
}

// ============================================================================
// AST (Abstract Syntax Tree)
// ============================================================================

/// Nodo del árbol de sintaxis
#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    String(String),
    CellRef(CellRef),
    Range(CellRange),
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    Negative(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// Parser de fórmulas
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Expr, FormulaError> {
        self.parse_comparison()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    fn parse_comparison(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_additive()?;

        while let Some(token) = self.peek() {
            let op = match token {
                Token::Equals => BinaryOp::Eq,
                Token::NotEquals => BinaryOp::Ne,
                Token::Greater => BinaryOp::Gt,
                Token::GreaterEq => BinaryOp::Ge,
                Token::Less => BinaryOp::Lt,
                Token::LessEq => BinaryOp::Le,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_multiplicative()?;

        while let Some(token) = self.peek() {
            let op = match token {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_unary()?;

        while let Some(token) = self.peek() {
            let op = match token {
                Token::Multiply => BinaryOp::Mul,
                Token::Divide => BinaryOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, FormulaError> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Negative(Box::new(expr)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, FormulaError> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Some(Token::String(s)) => {
                self.advance();
                Ok(Expr::String(s))
            }
            Some(Token::CellRef(cell)) => {
                self.advance();
                Ok(Expr::CellRef(cell))
            }
            Some(Token::Range(range)) => {
                self.advance();
                Ok(Expr::Range(range))
            }
            Some(Token::Function(name)) => {
                self.advance();
                self.parse_function_call(name)
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_comparison()?;
                if let Some(Token::RParen) = self.peek() {
                    self.advance();
                    Ok(expr)
                } else {
                    Err(FormulaError::MissingCloseParen)
                }
            }
            Some(token) => Err(FormulaError::UnexpectedToken(format!("{:?}", token))),
            None => Err(FormulaError::UnexpectedEnd),
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<Expr, FormulaError> {
        // Esperamos '('
        if let Some(Token::LParen) = self.peek() {
            self.advance();
        } else {
            return Err(FormulaError::ExpectedOpenParen);
        }

        let mut args = Vec::new();

        // Parsear argumentos
        if self.peek() != Some(&Token::RParen) {
            args.push(self.parse_comparison()?);
            
            while let Some(Token::Comma) = self.peek() {
                self.advance();
                args.push(self.parse_comparison()?);
            }
        }

        // Esperamos ')'
        if let Some(Token::RParen) = self.peek() {
            self.advance();
        } else {
            return Err(FormulaError::MissingCloseParen);
        }

        Ok(Expr::FunctionCall { name, args })
    }
}

// ============================================================================
// EVALUADOR
// ============================================================================

/// Valor de una celda
#[derive(Debug, Clone)]
pub enum CellValue {
    Number(f64),
    Text(String),
    Empty,
    Error(FormulaError),
}

impl std::fmt::Display for CellValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValue::Number(n) => {
                // Formatear número sin decimales innecesarios
                if n.fract() == 0.0 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            CellValue::Text(s) => write!(f, "{}", s),
            CellValue::Empty => write!(f, ""),
            CellValue::Error(e) => write!(f, "#ERROR: {:?}", e),
        }
    }
}

impl CellValue {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            CellValue::Number(n) => Some(*n),
            CellValue::Text(s) => s.parse().ok(),
            CellValue::Empty => Some(0.0),
            CellValue::Error(_) => None,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            CellValue::Number(n) => *n != 0.0,
            CellValue::Text(s) => !s.is_empty() && s.to_lowercase() != "false",
            CellValue::Empty => false,
            CellValue::Error(_) => false,
        }
    }
}

/// Grid de celdas para evaluación
pub struct CellGrid {
    /// Valores de las celdas (clave: "A1", "B2", etc.)
    cells: HashMap<String, CellValue>,
    /// Número máximo de filas
    max_row: u32,
    /// Número máximo de columnas
    max_col: u16,
}

impl CellGrid {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            max_row: 0,
            max_col: 0,
        }
    }

    /// Establecer valor de una celda
    pub fn set(&mut self, cell: CellRef, value: CellValue) {
        self.max_row = self.max_row.max(cell.row);
        self.max_col = self.max_col.max(cell.col);
        self.cells.insert(cell.to_string(), value);
    }

    /// Obtener valor de una celda
    pub fn get(&self, cell: &CellRef) -> CellValue {
        self.cells
            .get(&cell.to_string())
            .cloned()
            .unwrap_or(CellValue::Empty)
    }

    /// Evaluar una fórmula
    pub fn evaluate(&self, formula: &str) -> Result<CellValue, FormulaError> {
        let tokens = tokenize(formula)?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse()?;
        self.eval_expr(&expr)
    }

    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FormulaError> {
        match expr {
            Expr::Number(n) => Ok(CellValue::Number(*n)),
            Expr::String(s) => Ok(CellValue::Text(s.clone())),
            Expr::CellRef(cell) => Ok(self.get(cell)),
            Expr::Range(_) => Err(FormulaError::RangeNotAllowed),
            Expr::Negative(inner) => {
                let val = self.eval_expr(inner)?;
                match val.as_number() {
                    Some(n) => Ok(CellValue::Number(-n)),
                    None => Err(FormulaError::TypeMismatch("Expected number".to_string())),
                }
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                self.eval_binary_op(*op, l, r)
            }
            Expr::FunctionCall { name, args } => self.eval_function(name, args),
        }
    }

    fn eval_binary_op(&self, op: BinaryOp, left: CellValue, right: CellValue) -> Result<CellValue, FormulaError> {
        let l = left.as_number().ok_or(FormulaError::TypeMismatch("Left operand not a number".to_string()))?;
        let r = right.as_number().ok_or(FormulaError::TypeMismatch("Right operand not a number".to_string()))?;

        let result = match op {
            BinaryOp::Add => l + r,
            BinaryOp::Sub => l - r,
            BinaryOp::Mul => l * r,
            BinaryOp::Div => {
                if r == 0.0 {
                    return Err(FormulaError::DivisionByZero);
                }
                l / r
            }
            BinaryOp::Eq => if (l - r).abs() < f64::EPSILON { 1.0 } else { 0.0 },
            BinaryOp::Ne => if (l - r).abs() >= f64::EPSILON { 1.0 } else { 0.0 },
            BinaryOp::Gt => if l > r { 1.0 } else { 0.0 },
            BinaryOp::Ge => if l >= r { 1.0 } else { 0.0 },
            BinaryOp::Lt => if l < r { 1.0 } else { 0.0 },
            BinaryOp::Le => if l <= r { 1.0 } else { 0.0 },
        };

        Ok(CellValue::Number(result))
    }

    fn eval_function(&self, name: &str, args: &[Expr]) -> Result<CellValue, FormulaError> {
        match name {
            "SUM" => self.eval_aggregate(args, |vals| vals.iter().sum()),
            "AVG" | "AVERAGE" => self.eval_aggregate(args, |vals| {
                if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
            }),
            "MIN" => self.eval_aggregate(args, |vals| {
                vals.iter().cloned().fold(f64::INFINITY, f64::min)
            }),
            "MAX" => self.eval_aggregate(args, |vals| {
                vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            }),
            "COUNT" => self.eval_count(args),
            "COUNTA" => self.eval_counta(args),  // Contar celdas no vacías
            "IF" => {
                if args.len() != 3 {
                    return Err(FormulaError::WrongArgCount("IF".to_string(), 3, args.len()));
                }
                let cond = self.eval_expr(&args[0])?;
                if cond.as_bool() {
                    self.eval_expr(&args[1])
                } else {
                    self.eval_expr(&args[2])
                }
            }
            "ABS" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("ABS".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                match val.as_number() {
                    Some(n) => Ok(CellValue::Number(n.abs())),
                    None => Err(FormulaError::TypeMismatch("ABS requires number".to_string())),
                }
            }
            "ROUND" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(FormulaError::WrongArgCount("ROUND".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?.as_number()
                    .ok_or(FormulaError::TypeMismatch("ROUND requires number".to_string()))?;
                let decimals = if args.len() == 2 {
                    self.eval_expr(&args[1])?.as_number().unwrap_or(0.0) as i32
                } else {
                    0
                };
                let factor = 10f64.powi(decimals);
                Ok(CellValue::Number((val * factor).round() / factor))
            }
            // === Funciones de texto ===
            "CONCAT" | "CONCATENATE" => {
                let mut result = String::new();
                for arg in args {
                    let val = self.eval_expr(arg)?;
                    result.push_str(&val.to_string());
                }
                Ok(CellValue::Text(result))
            }
            "UPPER" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("UPPER".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                Ok(CellValue::Text(val.to_string().to_uppercase()))
            }
            "LOWER" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("LOWER".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                Ok(CellValue::Text(val.to_string().to_lowercase()))
            }
            "TRIM" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("TRIM".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                Ok(CellValue::Text(val.to_string().trim().to_string()))
            }
            "LEN" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("LEN".to_string(), 1, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                Ok(CellValue::Number(val.to_string().chars().count() as f64))
            }
            "LEFT" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(FormulaError::WrongArgCount("LEFT".to_string(), 2, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let num = if args.len() == 2 {
                    self.eval_expr(&args[1])?.as_number().unwrap_or(1.0) as usize
                } else {
                    1
                };
                Ok(CellValue::Text(text.chars().take(num).collect()))
            }
            "RIGHT" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(FormulaError::WrongArgCount("RIGHT".to_string(), 2, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let num = if args.len() == 2 {
                    self.eval_expr(&args[1])?.as_number().unwrap_or(1.0) as usize
                } else {
                    1
                };
                let chars: Vec<char> = text.chars().collect();
                let start = chars.len().saturating_sub(num);
                Ok(CellValue::Text(chars[start..].iter().collect()))
            }
            "MID" => {
                if args.len() != 3 {
                    return Err(FormulaError::WrongArgCount("MID".to_string(), 3, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let start = self.eval_expr(&args[1])?.as_number().unwrap_or(1.0) as usize;
                let num = self.eval_expr(&args[2])?.as_number().unwrap_or(1.0) as usize;
                let chars: Vec<char> = text.chars().collect();
                // MID usa índice base 1 como Excel
                let start_idx = start.saturating_sub(1);
                let end_idx = (start_idx + num).min(chars.len());
                Ok(CellValue::Text(chars[start_idx..end_idx].iter().collect()))
            }
            "REPLACE" => {
                if args.len() != 4 {
                    return Err(FormulaError::WrongArgCount("REPLACE".to_string(), 4, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let start = self.eval_expr(&args[1])?.as_number().unwrap_or(1.0) as usize;
                let num = self.eval_expr(&args[2])?.as_number().unwrap_or(0.0) as usize;
                let new_text = self.eval_expr(&args[3])?.to_string();
                let chars: Vec<char> = text.chars().collect();
                let start_idx = start.saturating_sub(1);
                let end_idx = (start_idx + num).min(chars.len());
                let mut result: String = chars[..start_idx].iter().collect();
                result.push_str(&new_text);
                result.push_str(&chars[end_idx..].iter().collect::<String>());
                Ok(CellValue::Text(result))
            }
            "SUBSTITUTE" => {
                if args.len() < 3 || args.len() > 4 {
                    return Err(FormulaError::WrongArgCount("SUBSTITUTE".to_string(), 3, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let old_text = self.eval_expr(&args[1])?.to_string();
                let new_text = self.eval_expr(&args[2])?.to_string();
                
                if args.len() == 4 {
                    // Reemplazar solo la n-ésima ocurrencia
                    let instance = self.eval_expr(&args[3])?.as_number().unwrap_or(1.0) as usize;
                    let mut result = String::new();
                    let mut count = 0;
                    let mut last_end = 0;
                    for (start, _) in text.match_indices(&old_text) {
                        count += 1;
                        if count == instance {
                            result.push_str(&text[last_end..start]);
                            result.push_str(&new_text);
                            last_end = start + old_text.len();
                            break;
                        }
                    }
                    result.push_str(&text[last_end..]);
                    if count < instance {
                        Ok(CellValue::Text(text)) // No se encontró la ocurrencia
                    } else {
                        Ok(CellValue::Text(result))
                    }
                } else {
                    // Reemplazar todas las ocurrencias
                    Ok(CellValue::Text(text.replace(&old_text, &new_text)))
                }
            }
            "TEXT" => {
                // Formato simple: TEXT(valor, formato) - por ahora solo número de decimales
                if args.len() != 2 {
                    return Err(FormulaError::WrongArgCount("TEXT".to_string(), 2, args.len()));
                }
                let val = self.eval_expr(&args[0])?;
                let format = self.eval_expr(&args[1])?.to_string();
                
                if let Some(n) = val.as_number() {
                    // Contar decimales en el formato (ej: "0.00" = 2 decimales)
                    let decimals = format.split('.').nth(1).map(|s| s.len()).unwrap_or(0);
                    Ok(CellValue::Text(format!("{:.prec$}", n, prec = decimals)))
                } else {
                    Ok(CellValue::Text(val.to_string()))
                }
            }
            "REPT" => {
                if args.len() != 2 {
                    return Err(FormulaError::WrongArgCount("REPT".to_string(), 2, args.len()));
                }
                let text = self.eval_expr(&args[0])?.to_string();
                let times = self.eval_expr(&args[1])?.as_number().unwrap_or(1.0) as usize;
                Ok(CellValue::Text(text.repeat(times)))
            }
            // === Funciones de fecha ===
            "TODAY" => {
                // Fecha actual en formato YYYY-MM-DD
                let now = chrono::Local::now();
                Ok(CellValue::Text(now.format("%Y-%m-%d").to_string()))
            }
            "NOW" => {
                // Fecha y hora actual
                let now = chrono::Local::now();
                Ok(CellValue::Text(now.format("%Y-%m-%d %H:%M").to_string()))
            }
            "YEAR" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("YEAR".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(year) = Self::parse_date_component(&date_str, "year") {
                    Ok(CellValue::Number(year as f64))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "MONTH" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("MONTH".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(month) = Self::parse_date_component(&date_str, "month") {
                    Ok(CellValue::Number(month as f64))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "DAY" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("DAY".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(day) = Self::parse_date_component(&date_str, "day") {
                    Ok(CellValue::Number(day as f64))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "HOUR" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("HOUR".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(hour) = Self::parse_date_component(&date_str, "hour") {
                    Ok(CellValue::Number(hour as f64))
                } else {
                    Ok(CellValue::Number(0.0))
                }
            }
            "MINUTE" => {
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("MINUTE".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(min) = Self::parse_date_component(&date_str, "minute") {
                    Ok(CellValue::Number(min as f64))
                } else {
                    Ok(CellValue::Number(0.0))
                }
            }
            "WEEKDAY" => {
                // 1=Lunes ... 7=Domingo (ISO)
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("WEEKDAY".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(wd) = Self::parse_date_component(&date_str, "weekday") {
                    Ok(CellValue::Number(wd as f64))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "WEEKNUM" => {
                // Número de semana del año
                if args.len() != 1 {
                    return Err(FormulaError::WrongArgCount("WEEKNUM".to_string(), 1, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                if let Some(wn) = Self::parse_date_component(&date_str, "weeknum") {
                    Ok(CellValue::Number(wn as f64))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "DATEDIF" => {
                // Diferencia entre dos fechas
                if args.len() != 3 {
                    return Err(FormulaError::WrongArgCount("DATEDIF".to_string(), 3, args.len()));
                }
                let date1_str = self.eval_expr(&args[0])?.to_string();
                let date2_str = self.eval_expr(&args[1])?.to_string();
                let unit = self.eval_expr(&args[2])?.to_string().to_uppercase();
                
                if let (Some(d1), Some(d2)) = (Self::parse_date(&date1_str), Self::parse_date(&date2_str)) {
                    let diff = d2.signed_duration_since(d1);
                    let result = match unit.as_str() {
                        "D" => diff.num_days() as f64,
                        "M" => (diff.num_days() / 30) as f64, // Aproximado
                        "Y" => (diff.num_days() / 365) as f64, // Aproximado
                        "H" => diff.num_hours() as f64,
                        _ => diff.num_days() as f64,
                    };
                    Ok(CellValue::Number(result))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "DATEFORMAT" => {
                // Formatear fecha: DATEFORMAT(fecha, "formato")
                if args.len() != 2 {
                    return Err(FormulaError::WrongArgCount("DATEFORMAT".to_string(), 2, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                let format = self.eval_expr(&args[1])?.to_string();
                
                if let Some(dt) = Self::parse_datetime(&date_str) {
                    // Convertir formato tipo Excel a chrono
                    let chrono_fmt = format
                        .replace("YYYY", "%Y")
                        .replace("YY", "%y")
                        .replace("MMMM", "%B")
                        .replace("MMM", "%b")
                        .replace("MM", "%m")
                        .replace("DD", "%d")
                        .replace("HH", "%H")
                        .replace("mm", "%M")
                        .replace("ss", "%S");
                    Ok(CellValue::Text(dt.format(&chrono_fmt).to_string()))
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            "EOMONTH" => {
                // Último día del mes (con offset de meses)
                if args.len() < 1 || args.len() > 2 {
                    return Err(FormulaError::WrongArgCount("EOMONTH".to_string(), 2, args.len()));
                }
                let date_str = self.eval_expr(&args[0])?.to_string();
                let months_offset = if args.len() == 2 {
                    self.eval_expr(&args[1])?.as_number().unwrap_or(0.0) as i32
                } else {
                    0
                };
                
                if let Some(dt) = Self::parse_date(&date_str) {
                    use chrono::{Datelike, NaiveDate};
                    let year = dt.year();
                    let month = dt.month() as i32;
                    
                    // Calcular nuevo mes/año con offset
                    let total_months = year * 12 + month - 1 + months_offset;
                    let new_year = total_months / 12;
                    let new_month = (total_months % 12 + 1) as u32;
                    
                    // Obtener último día del mes
                    let next_month = if new_month == 12 { 1 } else { new_month + 1 };
                    let next_year = if new_month == 12 { new_year + 1 } else { new_year };
                    if let Some(first_of_next) = NaiveDate::from_ymd_opt(next_year, next_month, 1) {
                        let last_day = first_of_next.pred_opt().unwrap_or(first_of_next);
                        Ok(CellValue::Text(last_day.format("%Y-%m-%d").to_string()))
                    } else {
                        Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                    }
                } else {
                    Ok(CellValue::Text("#FECHA_INVALIDA".to_string()))
                }
            }
            _ => Err(FormulaError::UnknownFunction(name.to_string())),
        }
    }

    fn eval_aggregate<F>(&self, args: &[Expr], f: F) -> Result<CellValue, FormulaError>
    where
        F: Fn(&[f64]) -> f64,
    {
        let mut values = Vec::new();

        for arg in args {
            match arg {
                Expr::Range(range) => {
                    for cell in range.cells(self.max_row.max(100)) {
                        if let Some(n) = self.get(&cell).as_number() {
                            values.push(n);
                        }
                    }
                }
                Expr::CellRef(cell) => {
                    if let Some(n) = self.get(cell).as_number() {
                        values.push(n);
                    }
                }
                _ => {
                    let val = self.eval_expr(arg)?;
                    if let Some(n) = val.as_number() {
                        values.push(n);
                    }
                }
            }
        }

        Ok(CellValue::Number(f(&values)))
    }
    
    /// COUNT - Cuenta celdas con números
    fn eval_count(&self, args: &[Expr]) -> Result<CellValue, FormulaError> {
        let mut count = 0;

        for arg in args {
            match arg {
                Expr::Range(range) => {
                    for cell in range.cells(self.max_row.max(1)) {
                        let val = self.get(&cell);
                        if matches!(val, CellValue::Number(_)) {
                            count += 1;
                        }
                    }
                }
                Expr::CellRef(cell) => {
                    let val = self.get(cell);
                    if matches!(val, CellValue::Number(_)) {
                        count += 1;
                    }
                }
                _ => {
                    let val = self.eval_expr(arg)?;
                    if matches!(val, CellValue::Number(_)) {
                        count += 1;
                    }
                }
            }
        }

        Ok(CellValue::Number(count as f64))
    }
    
    /// COUNTA - Cuenta celdas no vacías
    fn eval_counta(&self, args: &[Expr]) -> Result<CellValue, FormulaError> {
        let mut count = 0;

        for arg in args {
            match arg {
                Expr::Range(range) => {
                    for cell in range.cells(self.max_row.max(1)) {
                        let val = self.get(&cell);
                        if !matches!(val, CellValue::Empty) {
                            count += 1;
                        }
                    }
                }
                Expr::CellRef(cell) => {
                    let val = self.get(cell);
                    if !matches!(val, CellValue::Empty) {
                        count += 1;
                    }
                }
                _ => {
                    let val = self.eval_expr(arg)?;
                    if !matches!(val, CellValue::Empty) {
                        count += 1;
                    }
                }
            }
        }

        Ok(CellValue::Number(count as f64))
    }
    
    // === Funciones auxiliares para fechas ===
    
    /// Parsear fecha en varios formatos comunes
    fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
        use chrono::NaiveDate;
        let s = s.trim();
        
        // Intentar varios formatos
        let formats = [
            "%Y-%m-%d",        // 2024-12-01
            "%d/%m/%Y",        // 01/12/2024
            "%m/%d/%Y",        // 12/01/2024
            "%Y/%m/%d",        // 2024/12/01
            "%d-%m-%Y",        // 01-12-2024
            "%Y-%m-%d %H:%M",  // 2024-12-01 10:30
            "%Y-%m-%d %H:%M:%S", // 2024-12-01 10:30:45
            "%Y-%m-%dT%H:%M:%S", // ISO 8601
            "%Y-%m-%dT%H:%M:%S%.f", // ISO 8601 con milisegundos
        ];
        
        for fmt in &formats {
            // Intentar como fecha sola
            if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
                return Some(d);
            }
            // Intentar como datetime y extraer fecha
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
                return Some(dt.date());
            }
        }
        
        // Intentar extraer solo la parte de fecha si hay espacio
        if let Some(date_part) = s.split_whitespace().next() {
            for fmt in &["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"] {
                if let Ok(d) = NaiveDate::parse_from_str(date_part, fmt) {
                    return Some(d);
                }
            }
        }
        
        None
    }
    
    /// Parsear datetime completo
    fn parse_datetime(s: &str) -> Option<chrono::NaiveDateTime> {
        use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
        let s = s.trim();
        
        let formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M:%S%.f",
            "%d/%m/%Y %H:%M:%S",
            "%d/%m/%Y %H:%M",
        ];
        
        for fmt in &formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
                return Some(dt);
            }
        }
        
        // Si solo es fecha, agregar 00:00:00
        if let Some(d) = Self::parse_date(s) {
            return Some(NaiveDateTime::new(d, NaiveTime::from_hms_opt(0, 0, 0)?));
        }
        
        None
    }
    
    /// Extraer componente específico de una fecha
    fn parse_date_component(s: &str, component: &str) -> Option<i32> {
        use chrono::{Datelike, Timelike};
        
        // Primero intentar como datetime
        if let Some(dt) = Self::parse_datetime(s) {
            return match component {
                "year" => Some(dt.year()),
                "month" => Some(dt.month() as i32),
                "day" => Some(dt.day() as i32),
                "hour" => Some(dt.hour() as i32),
                "minute" => Some(dt.minute() as i32),
                "weekday" => Some(dt.weekday().number_from_monday() as i32),
                "weeknum" => Some(dt.iso_week().week() as i32),
                _ => None,
            };
        }
        
        // Luego intentar solo como fecha
        if let Some(d) = Self::parse_date(s) {
            return match component {
                "year" => Some(d.year()),
                "month" => Some(d.month() as i32),
                "day" => Some(d.day() as i32),
                "weekday" => Some(d.weekday().number_from_monday() as i32),
                "weeknum" => Some(d.iso_week().week() as i32),
                _ => None,
            };
        }
        
        None
    }
}

impl Default for CellGrid {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ERRORES
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaError {
    InvalidNumber(String),
    InvalidIdentifier(String),
    UnexpectedChar(char),
    UnexpectedToken(String),
    UnexpectedEnd,
    MissingCloseParen,
    ExpectedOpenParen,
    DivisionByZero,
    TypeMismatch(String),
    UnknownFunction(String),
    WrongArgCount(String, usize, usize),
    RangeNotAllowed,
    CircularReference,
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormulaError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            FormulaError::InvalidIdentifier(s) => write!(f, "Invalid identifier: {}", s),
            FormulaError::UnexpectedChar(c) => write!(f, "Unexpected character: {}", c),
            FormulaError::UnexpectedToken(s) => write!(f, "Unexpected token: {}", s),
            FormulaError::UnexpectedEnd => write!(f, "Unexpected end of formula"),
            FormulaError::MissingCloseParen => write!(f, "Missing closing parenthesis"),
            FormulaError::ExpectedOpenParen => write!(f, "Expected opening parenthesis"),
            FormulaError::DivisionByZero => write!(f, "Division by zero"),
            FormulaError::TypeMismatch(s) => write!(f, "Type mismatch: {}", s),
            FormulaError::UnknownFunction(s) => write!(f, "Unknown function: {}", s),
            FormulaError::WrongArgCount(f_name, expected, got) => {
                write!(f, "{} expects {} arguments, got {}", f_name, expected, got)
            }
            FormulaError::RangeNotAllowed => write!(f, "Range not allowed in this context"),
            FormulaError::CircularReference => write!(f, "Circular reference detected"),
        }
    }
}

impl std::error::Error for FormulaError {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_conversion() {
        assert_eq!(col_to_letters(0), "A");
        assert_eq!(col_to_letters(25), "Z");
        assert_eq!(col_to_letters(26), "AA");
        assert_eq!(col_to_letters(27), "AB");
        assert_eq!(col_to_letters(701), "ZZ");

        assert_eq!(col_from_letters("A"), Some(0));
        assert_eq!(col_from_letters("Z"), Some(25));
        assert_eq!(col_from_letters("AA"), Some(26));
        assert_eq!(col_from_letters("AB"), Some(27));
        assert_eq!(col_from_letters("ZZ"), Some(701));
    }

    #[test]
    fn test_cell_ref_parse() {
        assert_eq!(CellRef::parse("A1"), Some(CellRef::new(0, 1)));
        assert_eq!(CellRef::parse("B2"), Some(CellRef::new(1, 2)));
        assert_eq!(CellRef::parse("AA100"), Some(CellRef::new(26, 100)));
        assert_eq!(CellRef::parse("Z99"), Some(CellRef::new(25, 99)));
    }

    #[test]
    fn test_range_parse() {
        assert!(matches!(
            CellRange::parse("A1:C10"),
            Some(CellRange::Range { .. })
        ));
        assert!(matches!(
            CellRange::parse("B:B"),
            Some(CellRange::Column { col: 1 })
        ));
    }

    #[test]
    fn test_simple_formulas() {
        let grid = CellGrid::new();
        
        assert!(matches!(
            grid.evaluate("=1+2"),
            Ok(CellValue::Number(n)) if (n - 3.0).abs() < f64::EPSILON
        ));
        
        assert!(matches!(
            grid.evaluate("=10/2"),
            Ok(CellValue::Number(n)) if (n - 5.0).abs() < f64::EPSILON
        ));
        
        assert!(matches!(
            grid.evaluate("=(2+3)*4"),
            Ok(CellValue::Number(n)) if (n - 20.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn test_cell_references() {
        let mut grid = CellGrid::new();
        grid.set(CellRef::new(0, 1), CellValue::Number(10.0)); // A1
        grid.set(CellRef::new(1, 1), CellValue::Number(20.0)); // B1
        
        assert!(matches!(
            grid.evaluate("=A1+B1"),
            Ok(CellValue::Number(n)) if (n - 30.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn test_sum_function() {
        let mut grid = CellGrid::new();
        grid.set(CellRef::new(0, 1), CellValue::Number(1.0));
        grid.set(CellRef::new(0, 2), CellValue::Number(2.0));
        grid.set(CellRef::new(0, 3), CellValue::Number(3.0));
        
        assert!(matches!(
            grid.evaluate("=SUM(A1:A3)"),
            Ok(CellValue::Number(n)) if (n - 6.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn test_if_function() {
        let mut grid = CellGrid::new();
        grid.set(CellRef::new(0, 1), CellValue::Number(10.0));
        
        assert!(matches!(
            grid.evaluate("=IF(A1>5, 100, 0)"),
            Ok(CellValue::Number(n)) if (n - 100.0).abs() < f64::EPSILON
        ));
        
        assert!(matches!(
            grid.evaluate("=IF(A1<5, 100, 0)"),
            Ok(CellValue::Number(n)) if n.abs() < f64::EPSILON
        ));
    }
}
